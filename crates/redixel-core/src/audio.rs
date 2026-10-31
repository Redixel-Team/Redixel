//! Audio playback abstraction exposed to game code via [`GameContext`](crate::game::GameContext).
//!
//! Mirrors [`net`](crate::net): the trait and its handle types live here, free
//! of any dependency on a real audio backend, so `ctx.play_sound`/`ctx.play_music`
//! always compile. The concrete backends (`cpal` + `symphonia` natively, the
//! Web Audio API on wasm) live in the optional `redixel-audio` crate.

/// The lowest playback rate [`clamp_pitch`] lets through.
pub const MIN_PITCH: f32 = 0.1;

/// The highest playback rate [`clamp_pitch`] lets through.
pub const MAX_PITCH: f32 = 10.0;

/// Clamps a volume into `0.0..=1.0`, mapping a non-finite value to `0.0`.
///
/// A `NaN` summed into a mix silences every other sound in it, and the Web
/// Audio API throws on one, so every volume is clamped before it reaches a
/// backend.
pub fn clamp_volume(volume: f32) -> f32 {
    if volume.is_finite() {
        volume.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Clamps a playback rate into [`MIN_PITCH`]`..=`[`MAX_PITCH`], mapping a
/// non-finite value to `1.0`.
///
/// A rate of zero or below never advances through its clip, which would hold
/// a voice open forever.
pub fn clamp_pitch(pitch: f32) -> f32 {
    if pitch.is_finite() {
        pitch.clamp(MIN_PITCH, MAX_PITCH)
    } else {
        1.0
    }
}

/// Clamps a fade duration in seconds to be non-negative, mapping a non-finite
/// value to `0.0`.
pub fn clamp_fade(seconds: f32) -> f32 {
    if seconds.is_finite() { seconds.max(0.0) } else { 0.0 }
}

/// An opaque handle to an audio clip the engine has been asked to load.
///
/// Issued the moment a game calls `load_sound`/`load_sound_file`, before any
/// decoding happens. The same handle works
/// with both [`GameContext::play_sound`](crate::game::GameContext::play_sound)
/// (a one-shot effect) and
/// [`GameContext::play_music`](crate::game::GameContext::play_music) (a looped
/// track) — what differs is only how it is played, not what was loaded.
///
/// Only meaningful to the engine instance that issued it — never serialise one
/// or send it over the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundId(u32);

impl SoundId {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

/// A volume bus a sound is mixed through, independent of the master volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioChannel {
    Sfx,
    Music,
}

/// Master and per-channel volume, each `0.0` (silent) to `1.0` (unattenuated).
///
/// Plain data with no dependency on a real audio backend, so it is read from
/// `config.json` and carried by `RuntimeConfig` whether or not the `audio`
/// feature is enabled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioConfig {
    pub master_volume: f32,
    pub sfx_volume:    f32,
    pub music_volume:  f32,
}

impl AudioConfig {
    /// A copy with every volume passed through [`clamp_volume`].
    pub fn clamped(self) -> Self {
        Self {
            master_volume: clamp_volume(self.master_volume),
            sfx_volume:    clamp_volume(self.sfx_volume),
            music_volume:  clamp_volume(self.music_volume),
        }
    }

    /// The volume of `channel`.
    pub fn channel(&self, channel: AudioChannel) -> f32 {
        match channel {
            AudioChannel::Sfx => self.sfx_volume,
            AudioChannel::Music => self.music_volume,
        }
    }

    /// Sets the volume of `channel`.
    pub fn set_channel(&mut self, channel: AudioChannel, volume: f32) {
        match channel {
            AudioChannel::Sfx => self.sfx_volume = volume,
            AudioChannel::Music => self.music_volume = volume,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            sfx_volume:    1.0,
            music_volume:  1.0,
        }
    }
}

/// How [`GameContext::play_music_with`](crate::game::GameContext::play_music_with)
/// starts a track.
///
/// - `volume`: the track's own volume, `0.0` to `1.0`, multiplied with the
///   music channel and master volume.
/// - `fade_in`: seconds to ramp the track up from silence; `0.0` starts it at
///   full volume.
/// - `fade_out_previous`: seconds over which the outgoing track fades while
///   this one starts, the two overlapping so a switch leaves no gap of
///   silence; `0.0` cuts it within a few milliseconds.
/// - `looped`: whether the track restarts from the beginning when it ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicOptions {
    pub volume:            f32,
    pub fade_in:           f32,
    pub fade_out_previous: f32,
    pub looped:            bool,
}

impl MusicOptions {
    /// A copy with `volume` passed through [`clamp_volume`] and both fades
    /// through [`clamp_fade`].
    pub fn clamped(self) -> Self {
        Self {
            volume:            clamp_volume(self.volume),
            fade_in:           clamp_fade(self.fade_in),
            fade_out_previous: clamp_fade(self.fade_out_previous),
            looped:            self.looped,
        }
    }
}

impl Default for MusicOptions {
    fn default() -> Self {
        Self {
            volume:            1.0,
            fade_in:           0.0,
            fade_out_previous: 0.0,
            looped:            true,
        }
    }
}

/// The audio backend interface the runtime drives.
///
/// Game code never touches this directly — it goes through the buffered
/// `GameContext` methods (`play_sound`, `play_music`, ...), which the runtime
/// dispatches into whichever `AudioManager` it built, the same way
/// `DrawCommand`s dispatch into the `Renderer`. Implementations clamp every
/// volume, pitch and fade they receive with [`clamp_volume`], [`clamp_pitch`]
/// and [`clamp_fade`].
pub trait AudioManager {
    /// Registers `bytes` for decoding under `id` without blocking the caller.
    ///
    /// A play made against a still-decoding clip starts once it resolves. A
    /// clip that fails to decode plays silently forever after — the same
    /// "never hard-fail on a bad asset" contract textures already have.
    fn load_clip(&mut self, id: SoundId, bytes: Vec<u8>);

    /// Plays `clip` once through the [`Sfx`](AudioChannel::Sfx) channel.
    ///
    /// A play made while `clip` is still decoding starts once decoding
    /// finishes, unless the clip fails to decode or is still decoding a
    /// couple of seconds later.
    fn play_sound(&mut self, clip: SoundId, volume: f32, pitch: f32);

    /// Plays `clip` on the [`Music`](AudioChannel::Music) channel, replacing
    /// whatever is playing there, and cancels any earlier `play_music` still
    /// waiting on its clip to decode so an older request can never start
    /// after a newer one. A play made while `clip` is still decoding waits
    /// for it however long that takes.
    fn play_music(&mut self, clip: SoundId, options: MusicOptions);

    /// Fades the current music track to silence over `fade_out` seconds and
    /// stops it, cancelling any `play_music` still waiting on its clip.
    fn stop_music(&mut self, fade_out: f32);

    /// Sets the master volume, applied on top of both channels.
    fn set_master_volume(&mut self, volume: f32);

    /// Sets one channel's volume, independent of the other channel and of
    /// the master volume.
    fn set_channel_volume(&mut self, channel: AudioChannel, volume: f32);

    /// Silences output while the app is in the background. The runtime calls
    /// this when the OS suspends the app.
    fn suspend(&mut self) {}

    /// Restores output after [`suspend`](Self::suspend). The runtime calls
    /// this when the OS resumes the app.
    fn resume(&mut self) {}

    /// Collects finished decodes and starts the plays that were waiting on
    /// them, ageing the rest by `dt` seconds. The runtime calls this once per
    /// frame.
    fn update(&mut self, dt: f32);
}

/// A zero-cost [`AudioManager`] used when the `audio` feature is disabled or
/// the platform's audio device failed to open.
///
/// Installed as the default so `ctx.play_sound`/`ctx.play_music` are always
/// callable — a headless server or a build without the `audio` feature hits
/// no special-casing. The game still reads back the volumes it set, since
/// `GameContext` tracks them itself.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NoOpAudio;

impl AudioManager for NoOpAudio {
    #[inline]
    fn load_clip(&mut self, _id: SoundId, _bytes: Vec<u8>) {}

    #[inline]
    fn play_sound(&mut self, _clip: SoundId, _volume: f32, _pitch: f32) {}

    #[inline]
    fn play_music(&mut self, _clip: SoundId, _options: MusicOptions) {}

    #[inline]
    fn stop_music(&mut self, _fade_out: f32) {}

    #[inline]
    fn set_master_volume(&mut self, _volume: f32) {}

    #[inline]
    fn set_channel_volume(&mut self, _channel: AudioChannel, _volume: f32) {}

    #[inline]
    fn update(&mut self, _dt: f32) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_volume_keeps_the_unit_range_and_silences_non_finite_values() {
        assert_eq!(clamp_volume(0.25), 0.25);
        assert_eq!(clamp_volume(-1.0), 0.0);
        assert_eq!(clamp_volume(8.0), 1.0);
        assert_eq!(clamp_volume(f32::NAN), 0.0);
        assert_eq!(clamp_volume(f32::INFINITY), 0.0);
    }

    #[test]
    fn clamp_pitch_rejects_rates_that_would_never_advance() {
        assert_eq!(clamp_pitch(1.5), 1.5);
        assert_eq!(clamp_pitch(0.0), MIN_PITCH);
        assert_eq!(clamp_pitch(-3.0), MIN_PITCH);
        assert_eq!(clamp_pitch(500.0), MAX_PITCH);
        assert_eq!(clamp_pitch(f32::NAN), 1.0);
    }

    #[test]
    fn clamp_fade_is_never_negative() {
        assert_eq!(clamp_fade(2.0), 2.0);
        assert_eq!(clamp_fade(-1.0), 0.0);
        assert_eq!(clamp_fade(f32::NAN), 0.0);
    }

    #[test]
    fn audio_config_reads_and_writes_each_channel() {
        let mut config: AudioConfig = AudioConfig::default();
        config.set_channel(AudioChannel::Sfx, 0.3);
        config.set_channel(AudioChannel::Music, 0.7);

        assert_eq!(config.channel(AudioChannel::Sfx), 0.3);
        assert_eq!(config.channel(AudioChannel::Music), 0.7);
        assert_eq!(config.master_volume, 1.0);
    }

    #[test]
    fn music_options_default_loops_at_unity_volume_with_no_fade() {
        let options: MusicOptions = MusicOptions::default();

        assert_eq!(options.volume, 1.0);
        assert_eq!(options.fade_in, 0.0);
        assert_eq!(options.fade_out_previous, 0.0);
        assert!(options.looped);
    }

    #[test]
    fn music_options_clamped_sanitizes_volume_and_fades() {
        let options: MusicOptions = MusicOptions {
            volume:            2.0,
            fade_in:           -1.0,
            fade_out_previous: f32::NAN,
            looped:            false,
        }
        .clamped();

        assert_eq!(options.volume, 1.0);
        assert_eq!(options.fade_in, 0.0);
        assert_eq!(options.fade_out_previous, 0.0);
        assert!(!options.looped);
    }

    #[test]
    fn sound_id_round_trips_its_index() {
        let id: SoundId = SoundId::new(42);
        assert_eq!(id.index(), 42);
    }
}
