//! Web audio backend: the browser's Web Audio API through `web-sys`.
//!
//! Clips decode through `decodeAudioData`, which is asynchronous by
//! construction, and play through one graph built up front:
//! `source -> per-play gain -> channel gain -> master gain -> destination`.
//! Fades are scheduled on the `AudioContext`'s own clock, so they are
//! sample-accurate without being driven per frame.
//!
//! Browsers keep an `AudioContext` suspended until the page receives a user
//! gesture, so the context resumes on the first key press, click or touch.
//! Music started before then waits for it; sound effects played before then
//! are dropped instead of all bursting out at once when it arrives.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

use js_sys::{Promise, Uint8Array};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, AudioNode, AudioParam,
    AudioScheduledSourceNode, GainNode, Window,
};

use redixel_core::{
    AudioChannel, AudioConfig, AudioManager, ClipState, MusicOptions, SoundId,
    audio::{clamp_fade, clamp_pitch, clamp_volume},
};

use crate::{
    MIN_FADE_SECS,
    pending::{PendingPlays, ReadyPlay},
    registry::ClipRegistry,
};

/// The DOM events browsers accept as the user gesture that lets audio start.
const GESTURE_EVENTS: [&str; 4] = ["keydown", "mousedown", "pointerup", "touchend"];

/// Decodes that finished since the last `update`, pushed by `spawn_local`
/// tasks. wasm32 has no threads, so the tasks and `update` only interleave at
/// `.await` points and a plain `Rc<RefCell<..>>` suffices — the same idiom as
/// `redixel-net`'s web backend.
type Mailbox = Rc<RefCell<VecDeque<(SoundId, Option<AudioBuffer>)>>>;

/// The track playing on the music channel and the gain node its fades run on.
struct MusicVoice {
    source: AudioBufferSourceNode,
    gain: GainNode,
}

/// Window listeners that resume the `AudioContext` on a user gesture unless
/// the app itself is suspended, removed again when this is dropped.
struct GestureResume {
    window: Window,
    listener: Closure<dyn FnMut()>,
}

impl GestureResume {
    fn install(ctx: &AudioContext, app_suspended: Rc<Cell<bool>>) -> Option<Self> {
        let window: Window = web_sys::window()?;
        let ctx: AudioContext = ctx.clone();

        let listener: Closure<dyn FnMut()> = Closure::new(move || {
            if !app_suspended.get() && ctx.state() == AudioContextState::Suspended {
                ctx.resume().ok();
            }
        });

        for event in GESTURE_EVENTS {
            if let Err(e) = window.add_event_listener_with_callback(event, listener.as_ref().unchecked_ref()) {
                log::warn!("Failed to listen for `{event}` to start audio: {e:?}");
            }
        }

        Some(Self { window, listener })
    }
}

impl Drop for GestureResume {
    fn drop(&mut self) {
        for event in GESTURE_EVENTS {
            self.window
                .remove_event_listener_with_callback(event, self.listener.as_ref().unchecked_ref())
                .ok();
        }
    }
}

/// The web [`AudioManager`], playing through the page's `AudioContext`.
pub struct WasmAudioManager {
    ctx: AudioContext,
    master_gain: GainNode,
    sfx_gain: GainNode,
    music_gain: GainNode,
    volumes: AudioConfig,
    clips: ClipRegistry<AudioBuffer>,
    decoded: Mailbox,
    pending: PendingPlays,
    music: Option<MusicVoice>,
    app_suspended: Rc<Cell<bool>>,
    _gesture_resume: Option<GestureResume>,
}

impl WasmAudioManager {
    /// Creates the `AudioContext` and its gain graph at `config`'s volumes.
    ///
    /// Fails when the browser refuses to create the context or its nodes.
    pub fn new(config: &AudioConfig) -> Result<Self, JsValue> {
        let volumes: AudioConfig = config.clamped();
        let ctx: AudioContext = AudioContext::new()?;

        let master_gain: GainNode = connected_gain(&ctx, volumes.master_volume, &ctx.destination())?;
        let sfx_gain: GainNode = connected_gain(&ctx, volumes.sfx_volume, &master_gain)?;
        let music_gain: GainNode = connected_gain(&ctx, volumes.music_volume, &master_gain)?;

        let app_suspended: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let gesture_resume: Option<GestureResume> = GestureResume::install(&ctx, Rc::clone(&app_suspended));

        Ok(Self {
            ctx,
            master_gain,
            sfx_gain,
            music_gain,
            volumes,
            clips: ClipRegistry::default(),
            decoded: Rc::new(RefCell::new(VecDeque::new())),
            pending: PendingPlays::default(),
            music: None,
            app_suspended,
            _gesture_resume: gesture_resume,
        })
    }

    fn start_sound(&self, clip: SoundId, volume: f32, pitch: f32) {
        if self.ctx.state() != AudioContextState::Running {
            return;
        }

        if let Some(buffer) = self.clips.get(clip)
            && let Err(e) = play_buffer(&self.ctx, buffer, volume, pitch, &self.sfx_gain)
        {
            log::warn!("Failed to start sound {}: {e:?}", clip.index());
        }
    }

    fn start_music(&mut self, clip: SoundId, options: MusicOptions) {
        let Some(buffer) = self.clips.get(clip) else {
            return;
        };

        match loop_buffer(&self.ctx, buffer, options, &self.music_gain) {
            Ok(voice) => {
                if let Some(outgoing) = self.music.replace(voice) {
                    fade_out_voice(&self.ctx, &outgoing, options.fade_out_previous);
                }
            }
            Err(e) => log::warn!("Failed to start music {}: {e:?}", clip.index()),
        }
    }
}

impl AudioManager for WasmAudioManager {
    fn load_clip(&mut self, id: SoundId, bytes: Vec<u8>) {
        self.clips.begin(id);

        let ctx: AudioContext = self.ctx.clone();
        let decoded: Mailbox = Rc::clone(&self.decoded);

        wasm_bindgen_futures::spawn_local(async move {
            let buffer: Option<AudioBuffer> = match decode(&ctx, &bytes).await {
                Ok(buffer) => Some(buffer),
                Err(e) => {
                    log::warn!("Failed to decode sound {}: {e:?}. It will play silently.", id.index());
                    None
                }
            };
            decoded.borrow_mut().push_back((id, buffer));
        });
    }

    fn clip_state(&self, id: SoundId) -> ClipState {
        self.clips.state(id)
    }

    fn play_sound(&mut self, clip: SoundId, volume: f32, pitch: f32) {
        let volume: f32 = clamp_volume(volume);
        let pitch: f32 = clamp_pitch(pitch);

        match self.clips.state(clip) {
            ClipState::Ready => self.start_sound(clip, volume, pitch),
            ClipState::Pending => self.pending.wait_sound(clip, volume, pitch),
            ClipState::Failed => {}
        }
    }

    fn play_music(&mut self, clip: SoundId, options: MusicOptions) {
        let options: MusicOptions = options.clamped();
        self.pending.cancel_music();

        match self.clips.state(clip) {
            ClipState::Ready => self.start_music(clip, options),
            ClipState::Pending => self.pending.wait_music(clip, options),
            ClipState::Failed => {}
        }
    }

    fn stop_music(&mut self, fade_out: f32) {
        self.pending.cancel_music();

        if let Some(voice) = self.music.take() {
            fade_out_voice(&self.ctx, &voice, clamp_fade(fade_out));
        }
    }

    fn set_master_volume(&mut self, volume: f32) {
        self.volumes.master_volume = clamp_volume(volume);
        self.master_gain.gain().set_value(self.volumes.master_volume);
    }

    fn set_channel_volume(&mut self, channel: AudioChannel, volume: f32) {
        let volume: f32 = clamp_volume(volume);
        self.volumes.set_channel(channel, volume);

        let bus: &GainNode = match channel {
            AudioChannel::Sfx => &self.sfx_gain,
            AudioChannel::Music => &self.music_gain,
        };
        bus.gain().set_value(volume);
    }

    fn master_volume(&self) -> f32 {
        self.volumes.master_volume
    }

    fn channel_volume(&self, channel: AudioChannel) -> f32 {
        self.volumes.channel(channel)
    }

    fn suspend(&mut self) {
        self.app_suspended.set(true);
        self.ctx.suspend().ok();
    }

    fn resume(&mut self) {
        self.app_suspended.set(false);
        self.ctx.resume().ok();
    }

    fn update(&mut self, dt: f32) {
        let finished: VecDeque<(SoundId, Option<AudioBuffer>)> = self.decoded.take();
        for (id, buffer) in finished {
            self.clips.finish(id, buffer);
        }

        let clips: &ClipRegistry<AudioBuffer> = &self.clips;
        let ready: Vec<ReadyPlay> = self.pending.take_ready(dt, |id: SoundId| clips.state(id));

        for play in ready {
            match play {
                ReadyPlay::Sound { clip, volume, pitch } => self.start_sound(clip, volume, pitch),
                ReadyPlay::Music { clip, options } => self.start_music(clip, options),
            }
        }
    }
}

fn connected_gain(ctx: &AudioContext, volume: f32, destination: &AudioNode) -> Result<GainNode, JsValue> {
    let gain: GainNode = ctx.create_gain()?;
    gain.gain().set_value(volume);
    gain.connect_with_audio_node(destination)?;
    Ok(gain)
}

fn play_buffer(
    ctx: &AudioContext,
    buffer: &AudioBuffer,
    volume: f32,
    pitch: f32,
    bus: &GainNode,
) -> Result<(), JsValue> {
    let source: AudioBufferSourceNode = ctx.create_buffer_source()?;
    source.set_buffer(Some(buffer));
    source.playback_rate().set_value(pitch);

    let gain: GainNode = connected_gain(ctx, volume, bus)?;
    source.connect_with_audio_node(&gain)?;
    source.start()
}

fn loop_buffer(
    ctx: &AudioContext,
    buffer: &AudioBuffer,
    options: MusicOptions,
    bus: &GainNode,
) -> Result<MusicVoice, JsValue> {
    let source: AudioBufferSourceNode = ctx.create_buffer_source()?;
    source.set_buffer(Some(buffer));
    source.set_loop(options.looped);

    let gain: GainNode = connected_gain(ctx, options.volume, bus)?;
    if options.fade_in > 0.0 {
        let now: f64 = ctx.current_time();
        gain.gain().set_value_at_time(0.0, now)?;
        gain.gain()
            .linear_ramp_to_value_at_time(options.volume, now + f64::from(options.fade_in))?;
    }

    source.connect_with_audio_node(&gain)?;
    source.start()?;
    Ok(MusicVoice { source, gain })
}

/// Fades `voice` out over `seconds` and stops it once silent, stopping it at
/// once if the fade cannot be scheduled. The graph carries both through on
/// its own, so `voice` can be dropped as soon as this returns.
fn fade_out_voice(ctx: &AudioContext, voice: &MusicVoice, seconds: f32) {
    if let Err(e) = schedule_fade_out(ctx, voice, seconds) {
        log::warn!("Failed to fade out the music track: {e:?}");
        AudioScheduledSourceNode::stop(&voice.source).ok();
    }
}

/// Ramps `voice` from its current gain to silence, cancelling any ramp still
/// running on it first — a fade-in left scheduled would otherwise pull the
/// gain back up partway through the fade-out.
fn schedule_fade_out(ctx: &AudioContext, voice: &MusicVoice, seconds: f32) -> Result<(), JsValue> {
    let now: f64 = ctx.current_time();
    let end: f64 = now + f64::from(seconds.max(MIN_FADE_SECS));
    let gain: AudioParam = voice.gain.gain();
    let current: f32 = gain.value();

    gain.cancel_scheduled_values(now)?;
    gain.set_value_at_time(current, now)?;
    gain.linear_ramp_to_value_at_time(0.0, end)?;
    AudioScheduledSourceNode::stop_with_when(&voice.source, end)
}

async fn decode(ctx: &AudioContext, bytes: &[u8]) -> Result<AudioBuffer, JsValue> {
    let promise: Promise = ctx.decode_audio_data(&Uint8Array::from(bytes).buffer())?;
    let buffer: JsValue = JsFuture::from(promise).await?;
    Ok(buffer.unchecked_into::<AudioBuffer>())
}
