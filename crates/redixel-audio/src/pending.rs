//! Plays issued against a clip that was still decoding, held until it
//! resolves. Shared by the native and web backends so the policy cannot drift
//! between them.

use redixel_core::{ClipState, MusicOptions, SoundId};

/// How long a play may wait on a still-decoding clip before it is dropped, so
/// a sound never fires long after the moment that triggered it.
const MAX_WAIT_SECS: f32 = 2.0;

/// How many sound plays may wait at once. More would release a burst of stale
/// copies the moment a slow decode resolves.
const MAX_WAITING_SOUNDS: usize = 32;

/// A play whose clip has finished decoding and can start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ReadyPlay {
    Sound { clip: SoundId, volume: f32, pitch: f32 },
    Music { clip: SoundId, options: MusicOptions },
}

impl ReadyPlay {
    fn clip(&self) -> SoundId {
        match *self {
            ReadyPlay::Sound { clip, .. } | ReadyPlay::Music { clip, .. } => clip,
        }
    }
}

struct Waiting {
    play: ReadyPlay,
    age: f32,
}

/// Plays waiting on their clip to decode.
///
/// At most one music play waits at a time, and a newer `play_music` or a
/// `stop_music` replaces or cancels it, so an older music request can never
/// start after a newer one.
#[derive(Default)]
pub(crate) struct PendingPlays {
    sounds: Vec<Waiting>,
    music: Option<Waiting>,
}

impl PendingPlays {
    /// Holds a sound play until its clip resolves, dropping it if
    /// [`MAX_WAITING_SOUNDS`] are already waiting.
    pub(crate) fn wait_sound(&mut self, clip: SoundId, volume: f32, pitch: f32) {
        if self.sounds.len() < MAX_WAITING_SOUNDS {
            self.sounds.push(Waiting {
                play: ReadyPlay::Sound { clip, volume, pitch },
                age: 0.0,
            });
        }
    }

    /// Holds a music play until its clip resolves, replacing any music play
    /// already waiting.
    pub(crate) fn wait_music(&mut self, clip: SoundId, options: MusicOptions) {
        self.music = Some(Waiting {
            play: ReadyPlay::Music { clip, options },
            age: 0.0,
        });
    }

    /// Drops the waiting music play, if any.
    pub(crate) fn cancel_music(&mut self) {
        self.music = None;
    }

    /// Ages every waiting play by `dt` seconds and removes the ones whose clip
    /// resolved or that waited too long, returning those whose clip is ready.
    /// `state` reports each clip's current [`ClipState`].
    pub(crate) fn take_ready(&mut self, dt: f32, state: impl Fn(SoundId) -> ClipState) -> Vec<ReadyPlay> {
        let mut ready: Vec<ReadyPlay> = Vec::new();

        self.sounds
            .retain_mut(|waiting: &mut Waiting| keep_waiting(waiting, dt, &state, &mut ready));

        if let Some(waiting) = &mut self.music
            && !keep_waiting(waiting, dt, &state, &mut ready)
        {
            self.music = None;
        }

        ready
    }
}

/// Settles one waiting play, returning whether it should keep waiting. A play
/// whose clip is ready is pushed onto `ready`.
fn keep_waiting(
    waiting: &mut Waiting,
    dt: f32,
    state: &impl Fn(SoundId) -> ClipState,
    ready: &mut Vec<ReadyPlay>,
) -> bool {
    let clip: SoundId = waiting.play.clip();

    match state(clip) {
        ClipState::Ready => {
            ready.push(waiting.play);
            false
        }
        ClipState::Failed => false,
        ClipState::Pending => {
            waiting.age += dt;
            if waiting.age < MAX_WAIT_SECS {
                true
            } else {
                log::warn!(
                    "A play of sound {} was dropped after waiting {MAX_WAIT_SECS}s for it to decode.",
                    clip.index()
                );
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIP: SoundId = SoundId::new(0);
    const OTHER: SoundId = SoundId::new(1);

    fn sound(clip: SoundId) -> ReadyPlay {
        ReadyPlay::Sound {
            clip,
            volume: 1.0,
            pitch: 1.0,
        }
    }

    #[test]
    fn a_sound_starts_once_its_clip_is_ready() {
        let mut pending: PendingPlays = PendingPlays::default();
        pending.wait_sound(CLIP, 1.0, 1.0);

        assert!(pending.take_ready(0.1, |_: SoundId| ClipState::Pending).is_empty());
        assert_eq!(pending.take_ready(0.1, |_: SoundId| ClipState::Ready), vec![sound(CLIP)]);
        assert!(
            pending.take_ready(0.1, |_: SoundId| ClipState::Ready).is_empty(),
            "a started play must not start again"
        );
    }

    #[test]
    fn a_play_on_a_failed_clip_is_dropped() {
        let mut pending: PendingPlays = PendingPlays::default();
        pending.wait_sound(CLIP, 1.0, 1.0);

        assert!(pending.take_ready(0.1, |_: SoundId| ClipState::Failed).is_empty());
        assert!(pending.take_ready(0.1, |_: SoundId| ClipState::Ready).is_empty());
    }

    #[test]
    fn a_play_that_waits_too_long_is_dropped() {
        let mut pending: PendingPlays = PendingPlays::default();
        pending.wait_sound(CLIP, 1.0, 1.0);

        assert!(
            pending
                .take_ready(MAX_WAIT_SECS, |_: SoundId| ClipState::Pending)
                .is_empty()
        );
        assert!(pending.take_ready(0.1, |_: SoundId| ClipState::Ready).is_empty());
    }

    #[test]
    fn a_newer_music_play_replaces_the_waiting_one() {
        let mut pending: PendingPlays = PendingPlays::default();
        pending.wait_music(CLIP, MusicOptions::default());
        pending.wait_music(OTHER, MusicOptions::default());

        assert_eq!(
            pending.take_ready(0.1, |_: SoundId| ClipState::Ready),
            vec![ReadyPlay::Music {
                clip: OTHER,
                options: MusicOptions::default()
            }]
        );
    }

    #[test]
    fn cancelling_music_drops_its_waiting_play() {
        let mut pending: PendingPlays = PendingPlays::default();
        pending.wait_music(CLIP, MusicOptions::default());
        pending.cancel_music();

        assert!(pending.take_ready(0.1, |_: SoundId| ClipState::Ready).is_empty());
    }

    #[test]
    fn sound_plays_beyond_the_cap_are_dropped() {
        let mut pending: PendingPlays = PendingPlays::default();
        for _ in 0..MAX_WAITING_SOUNDS + 5 {
            pending.wait_sound(CLIP, 1.0, 1.0);
        }

        assert_eq!(pending.take_ready(0.1, |_: SoundId| ClipState::Ready).len(), MAX_WAITING_SOUNDS);
    }

    #[test]
    fn only_plays_on_resolved_clips_are_taken() {
        let mut pending: PendingPlays = PendingPlays::default();
        pending.wait_sound(CLIP, 1.0, 1.0);
        pending.wait_sound(OTHER, 1.0, 1.0);

        assert_eq!(
            pending.take_ready(0.1, |id: SoundId| if id == CLIP {
                ClipState::Ready
            } else {
                ClipState::Pending
            }),
            vec![sound(CLIP)]
        );
        assert_eq!(pending.take_ready(0.1, |_: SoundId| ClipState::Ready), vec![sound(OTHER)]);
    }
}
