//! Presenting swapchain images on a dedicated thread.

use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use hal::Queue as _;
use redixel_core::RedixelError;
use wgpu_hal as hal;

use super::api::gpu_err;

/// How long a spinning join polls before it falls back to blocking.
const SPIN_BUDGET: Duration = Duration::from_millis(2);

/// What the present thread reports back for each image.
pub(super) type PresentResult = Result<(), hal::SurfaceError>;

/// One image to present, with shared handles to the queue and surface it
/// belongs to, held only for the duration of the present.
pub(super) struct PresentJob<A: hal::Api> {
    pub(super) queue:   Arc<A::Queue>,
    pub(super) surface: Arc<A::Surface>,
    pub(super) texture: A::SurfaceTexture,
}

/// Presents swapchain images on a dedicated thread, so the game's next frame
/// runs while the present call blocks in the driver and compositor.
///
/// Queue operations must be externally synchronised, so at most one present is
/// in flight, and the frame thread [`join`](Self::join)s it before its next
/// acquire, submit or idle wait. The worker releases its queue and surface
/// handles before reporting, so after a join the frame thread holds the only
/// references again.
pub(super) struct Presenter<A: hal::Api> {
    jobs:    Option<Sender<PresentJob<A>>>,
    done:    Receiver<PresentResult>,
    pending: bool,
    spin:    bool,
    thread:  Option<JoinHandle<()>>,
}

impl<A: hal::Api> Presenter<A> {
    /// Starts the present thread. With `spin`, [`join`](Self::join) busy-waits
    /// for up to [`SPIN_BUDGET`] before blocking, which saves the wake-up
    /// latency at the cost of a busy core; the present thread itself always
    /// blocks, since spinning there starves the frame thread.
    pub(super) fn spawn(spin: bool) -> Result<Self, RedixelError> {
        let (jobs, inbox): (Sender<PresentJob<A>>, Receiver<PresentJob<A>>) = mpsc::channel();
        let (report, done): (Sender<PresentResult>, Receiver<PresentResult>) = mpsc::channel();

        let thread: JoinHandle<()> = thread::Builder::new()
            .name(String::from("redixel-present"))
            .spawn(move || {
                for job in inbox {
                    let PresentJob {
                        queue,
                        surface,
                        texture,
                    } = job;
                    // SAFETY: the frame thread queues a job only after
                    // submitting the work that renders `texture`, and touches
                    // neither the queue nor the swapchain again until it has
                    // received this job's result.
                    let result: PresentResult = unsafe { queue.present(&surface, texture) };
                    drop(queue);
                    drop(surface);
                    if report.send(result).is_err() {
                        break;
                    }
                }
            })
            .map_err(gpu_err)?;

        Ok(Self {
            jobs: Some(jobs),
            done,
            pending: false,
            spin,
            thread: Some(thread),
        })
    }

    pub(super) fn submit(&mut self, job: PresentJob<A>) -> Result<(), RedixelError> {
        let jobs: &Sender<PresentJob<A>> = self.jobs.as_ref().ok_or_else(|| gpu_err("present thread is stopped"))?;
        jobs.send(job)
            .map_err(|_: mpsc::SendError<PresentJob<A>>| gpu_err("present thread is stopped"))?;
        self.pending = true;
        Ok(())
    }

    /// Waits for the present in flight, if any, and returns its result.
    pub(super) fn join(&mut self) -> PresentResult {
        if !std::mem::take(&mut self.pending) {
            return Ok(());
        }

        if self.spin {
            let started: Instant = Instant::now();
            while started.elapsed() < SPIN_BUDGET {
                match self.done.try_recv() {
                    Ok(result) => return result,
                    Err(TryRecvError::Disconnected) => break,
                    Err(TryRecvError::Empty) => std::hint::spin_loop(),
                }
            }
        }

        self.done
            .recv()
            .unwrap_or(Err(hal::SurfaceError::Other("present thread stopped")))
    }
}

impl<A: hal::Api> Drop for Presenter<A> {
    fn drop(&mut self) {
        let _ = self.join();
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
