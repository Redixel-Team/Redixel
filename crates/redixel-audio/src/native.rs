//! Native audio backend: `cpal` opens the default output device, `symphonia`
//! decodes, and a multi-voice mixer runs inside `cpal`'s output callback.
//!
//! Clips decode in full on one dedicated thread and are resampled once to the
//! device's rate, so the callback only ever sums samples. The callback must
//! never block: plays and stops reach it through a lock-free ring buffer,
//! volumes are read from atomics, and its buffers are reserved before the
//! stream starts. Fades advance one output frame at a time inside the
//! callback, so they are sample-accurate whatever the game's frame rate.

use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};
use symphonia::core::{
    audio::{AudioSpec, GenericAudioBufferRef},
    codecs::{
        CodecParameters,
        audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions},
    },
    errors::Error as SymphoniaError,
    formats::{FormatOptions, FormatReader, Track, TrackType, probe::Hint},
    io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
};

use redixel_core::{
    AudioChannel, AudioConfig, AudioManager, ClipState, MusicOptions, RedixelError, SoundId,
    audio::{clamp_fade, clamp_pitch, clamp_volume},
};

use crate::{
    MIN_FADE_SECS,
    pending::{PendingPlays, ReadyPlay},
    registry::ClipRegistry,
};

/// Plays and stops that may queue up for the output callback between two of
/// its runs. A command beyond this is dropped with a warning rather than
/// blocking the game.
const COMMAND_QUEUE_CAPACITY: usize = 256;

/// Sound effects mixed at once. A play beyond this is dropped, bounding the
/// callback's work under a burst of plays.
const MAX_SFX_VOICES: usize = 32;

/// Music voices mixed at once: the current track and, during a crossfade, the
/// one fading out.
const MAX_MUSIC_VOICES: usize = 2;

/// Frames of mixing scratch reserved per output channel before the stream
/// starts, covering the buffer sizes output devices typically request.
const SCRATCH_FRAMES: usize = 4096;

type DecodeRequest = (SoundId, Vec<u8>);
type DecodeResult = (SoundId, Result<DecodedClip, RedixelError>);

/// A clip decoded in full: interleaved samples, `channels` per frame, at the
/// output stream's sample rate.
struct DecodedClip {
    samples: Box<[f32]>,
    channels: usize,
}

impl DecodedClip {
    fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }
}

/// An encoded file held in memory, readable by `symphonia`.
struct ByteSource(Cursor<Vec<u8>>);

impl Read for ByteSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for ByteSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl MediaSource for ByteSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.0.get_ref().len() as u64)
    }
}

/// Decodes `bytes` (WAV or OGG Vorbis) in full and resamples the result to
/// `sample_rate`.
fn decode_clip(bytes: Vec<u8>, sample_rate: u32) -> Result<DecodedClip, RedixelError> {
    let source: MediaSourceStream<'static> =
        MediaSourceStream::new(Box::new(ByteSource(Cursor::new(bytes))), MediaSourceStreamOptions::default());

    let mut format: Box<dyn FormatReader> = symphonia::default::get_probe()
        .probe(&Hint::new(), source, FormatOptions::default(), MetadataOptions::default())
        .map_err(decode_error)?;

    let track: &Track = format
        .first_track_known_codec(TrackType::Audio)
        .ok_or_else(|| RedixelError::AudioDecode(String::from("no audio track")))?;
    let track_id: u32 = track.id;
    let params: &AudioCodecParameters = track
        .codec_params
        .as_ref()
        .and_then(CodecParameters::audio)
        .ok_or_else(|| RedixelError::AudioDecode(String::from("no audio codec parameters")))?;

    let mut decoder: Box<dyn AudioDecoder> = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(decode_error)?;

    let mut samples: Vec<f32> = Vec::new();
    let mut packet_samples: Vec<f32> = Vec::new();
    let mut spec: Option<(u32, usize)> = None;

    while let Some(packet) = format.next_packet().map_err(decode_error)? {
        if packet.track_id != track_id {
            continue;
        }

        let buffer: GenericAudioBufferRef<'_> = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(SymphoniaError::DecodeError(reason)) => {
                log::warn!("Skipped a corrupt audio packet: {reason}");
                continue;
            }
            Err(e) => return Err(decode_error(e)),
        };

        let buffer_spec: &AudioSpec = buffer.spec();
        spec.get_or_insert((buffer_spec.rate(), buffer_spec.channels().count().max(1)));

        buffer.copy_to_vec_interleaved(&mut packet_samples);
        samples.extend_from_slice(&packet_samples);
    }

    let (source_rate, channels): (u32, usize) = match spec {
        Some(spec) if !samples.is_empty() => spec,
        _ => return Err(RedixelError::AudioDecode(String::from("no audio frames"))),
    };

    Ok(DecodedClip {
        samples: resample_linear(&samples, channels, source_rate, sample_rate).into_boxed_slice(),
        channels,
    })
}

fn decode_error(error: SymphoniaError) -> RedixelError {
    RedixelError::AudioDecode(error.to_string())
}

/// Resamples interleaved `input` from `from_rate` to `to_rate` by linear
/// interpolation.
fn resample_linear(input: &[f32], channels: usize, from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || from_rate == 0 || to_rate == 0 || input.is_empty() {
        return input.to_vec();
    }

    let frames_in: usize = input.len() / channels;
    let step: f64 = f64::from(from_rate) / f64::from(to_rate);
    let frames_out: usize = (frames_in as f64 / step).round() as usize;
    let mut output: Vec<f32> = Vec::with_capacity(frames_out * channels);

    for frame in 0..frames_out {
        let position: f64 = frame as f64 * step;
        let index: usize = (position as usize).min(frames_in - 1);
        let next: usize = (index + 1).min(frames_in - 1);
        let t: f32 = (position - index as f64) as f32;

        for channel in 0..channels {
            let a: f32 = input[index * channels + channel];
            let b: f32 = input[next * channels + channel];
            output.push(a + (b - a) * t);
        }
    }

    output
}

/// Spawns the thread every clip decodes on, returning the sender that feeds it
/// requests and the receiver its results come back on. The thread exits once
/// the request sender is dropped.
fn spawn_decoder(sample_rate: u32) -> Result<(Sender<DecodeRequest>, Receiver<DecodeResult>), RedixelError> {
    let (request_tx, request_rx): (Sender<DecodeRequest>, Receiver<DecodeRequest>) = mpsc::channel();
    let (result_tx, result_rx): (Sender<DecodeResult>, Receiver<DecodeResult>) = mpsc::channel();

    thread::Builder::new()
        .name(String::from("redixel-audio-decode"))
        .spawn(move || {
            for (id, bytes) in request_rx {
                if result_tx.send((id, decode_clip(bytes, sample_rate))).is_err() {
                    break;
                }
            }
        })?;

    Ok((request_tx, result_rx))
}

/// Master and channel volume, written by the game thread and read by the
/// output callback without locking.
struct SharedVolumes {
    master: AtomicU32,
    sfx: AtomicU32,
    music: AtomicU32,
}

impl SharedVolumes {
    fn new(volumes: AudioConfig) -> Self {
        Self {
            master: AtomicU32::new(volumes.master_volume.to_bits()),
            sfx: AtomicU32::new(volumes.sfx_volume.to_bits()),
            music: AtomicU32::new(volumes.music_volume.to_bits()),
        }
    }

    fn load(&self) -> AudioConfig {
        AudioConfig {
            master_volume: f32::from_bits(self.master.load(Ordering::Relaxed)),
            sfx_volume: f32::from_bits(self.sfx.load(Ordering::Relaxed)),
            music_volume: f32::from_bits(self.music.load(Ordering::Relaxed)),
        }
    }

    fn store_master(&self, volume: f32) {
        self.master.store(volume.to_bits(), Ordering::Relaxed);
    }

    fn store_channel(&self, channel: AudioChannel, volume: f32) {
        let target: &AtomicU32 = match channel {
            AudioChannel::Sfx => &self.sfx,
            AudioChannel::Music => &self.music,
        };
        target.store(volume.to_bits(), Ordering::Relaxed);
    }
}

/// What happens to a voice once its [`Fade`] reaches its target.
///
/// - `Settle`: it keeps playing at the target volume.
/// - `Stop`: it is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FadeEnd {
    Settle,
    Stop,
}

/// A linear volume ramp from `from` to `to` over `frames` output frames.
struct Fade {
    from: f32,
    to: f32,
    elapsed: u64,
    frames: u64,
    end: FadeEnd,
}

impl Fade {
    fn new(from: f32, to: f32, seconds: f32, sample_rate: u32, end: FadeEnd) -> Self {
        let frames: u64 = (f64::from(seconds) * f64::from(sample_rate)).round() as u64;

        Self {
            from,
            to,
            elapsed: 0,
            frames: frames.max(1),
            end,
        }
    }

    fn gain(&self) -> f32 {
        let t: f32 = (self.elapsed as f32 / self.frames as f32).min(1.0);
        self.from + (self.to - self.from) * t
    }

    /// Moves one frame along the ramp, returning `true` once it has arrived.
    fn advance(&mut self) -> bool {
        self.elapsed += 1;
        self.elapsed >= self.frames
    }
}

/// One playing clip.
///
/// `position` is a fractional frame index, so a pitch other than `1.0` steps
/// through the clip between frames. `volume` is the voice's gain whenever no
/// `fade` is running.
struct Voice {
    clip: Arc<DecodedClip>,
    channel: AudioChannel,
    position: f64,
    pitch: f64,
    volume: f32,
    looped: bool,
    fade: Option<Fade>,
}

impl Voice {
    fn gain(&self) -> f32 {
        self.fade.as_ref().map_or(self.volume, Fade::gain)
    }

    fn is_stopping(&self) -> bool {
        self.fade.as_ref().is_some_and(|fade: &Fade| fade.end == FadeEnd::Stop)
    }

    /// Fades the voice out from its current gain, however far into another
    /// fade it is.
    fn fade_out(&mut self, seconds: f32, sample_rate: u32) {
        self.fade = Some(Fade::new(
            self.gain(),
            0.0,
            seconds.max(MIN_FADE_SECS),
            sample_rate,
            FadeEnd::Stop,
        ));
    }

    /// Advances one output frame, returning `false` once the voice has
    /// finished.
    fn step(&mut self) -> bool {
        let arrived: Option<(FadeEnd, f32)> = self
            .fade
            .as_mut()
            .and_then(|fade: &mut Fade| fade.advance().then_some((fade.end, fade.to)));

        match arrived {
            Some((FadeEnd::Stop, _)) => return false,
            Some((FadeEnd::Settle, volume)) => {
                self.volume = volume;
                self.fade = None;
            }
            None => {}
        }

        let frames: f64 = self.clip.frames() as f64;
        self.position += self.pitch;

        if self.position >= frames {
            if !self.looped {
                return false;
            }
            self.position %= frames;
        }

        true
    }
}

/// A play or stop crossing from the game thread into the output callback,
/// carrying the decoded clip itself so the callback never reads the clip
/// registry.
enum MixerCommand {
    PlaySound {
        clip: Arc<DecodedClip>,
        volume: f32,
        pitch: f32,
    },
    PlayMusic {
        clip: Arc<DecodedClip>,
        options: MusicOptions,
    },
    StopMusic {
        fade_out: f32,
    },
}

fn apply_command(voices: &mut Vec<Voice>, command: MixerCommand, sample_rate: u32) {
    match command {
        MixerCommand::PlaySound { clip, volume, pitch } => {
            let playing: usize = voices
                .iter()
                .filter(|voice: &&Voice| voice.channel == AudioChannel::Sfx)
                .count();

            if playing < MAX_SFX_VOICES {
                voices.push(Voice {
                    clip,
                    channel: AudioChannel::Sfx,
                    position: 0.0,
                    pitch: f64::from(pitch),
                    volume,
                    looped: false,
                    fade: None,
                });
            }
        }
        MixerCommand::PlayMusic { clip, options } => {
            stop_music_voices(voices, options.fade_out_previous, sample_rate);

            let fade: Option<Fade> = (options.fade_in > 0.0)
                .then(|| Fade::new(0.0, options.volume, options.fade_in, sample_rate, FadeEnd::Settle));

            voices.push(Voice {
                clip,
                channel: AudioChannel::Music,
                position: 0.0,
                pitch: 1.0,
                volume: options.volume,
                looped: options.looped,
                fade,
            });
        }
        MixerCommand::StopMusic { fade_out } => stop_music_voices(voices, fade_out, sample_rate),
    }
}

/// Fades out every music voice over `fade_out` seconds, first removing the
/// ones already fading out, so at most one outgoing track ever overlaps the
/// next.
fn stop_music_voices(voices: &mut Vec<Voice>, fade_out: f32, sample_rate: u32) {
    voices.retain(|voice: &Voice| voice.channel != AudioChannel::Music || !voice.is_stopping());

    for voice in voices
        .iter_mut()
        .filter(|voice: &&mut Voice| voice.channel == AudioChannel::Music)
    {
        voice.fade_out(fade_out, sample_rate);
    }
}

/// Mixes every voice into `out`, interleaved over `out_channels` channels,
/// and removes the voices that finish.
///
/// A mono clip plays on every output channel. A clip with more channels maps
/// onto the output channel by channel, and output channels it has no source
/// for stay silent.
fn mix_into(voices: &mut Vec<Voice>, volumes: AudioConfig, out_channels: usize, out: &mut [f32]) {
    out.fill(0.0);
    if out_channels == 0 {
        return;
    }

    voices.retain_mut(|voice: &mut Voice| {
        let frames: usize = voice.clip.frames();
        if frames == 0 {
            return false;
        }

        let clip_channels: usize = voice.clip.channels;
        let bus: f32 = volumes.master_volume * volumes.channel(voice.channel);

        for frame in out.chunks_exact_mut(out_channels) {
            let index: usize = voice.position as usize;
            let next: usize = if index + 1 < frames {
                index + 1
            } else if voice.looped {
                0
            } else {
                index
            };
            let t: f32 = (voice.position - index as f64) as f32;
            let gain: f32 = voice.gain() * bus;

            for (channel, sample) in frame.iter_mut().enumerate() {
                let source: usize = if clip_channels == 1 {
                    0
                } else if channel < clip_channels {
                    channel
                } else {
                    continue;
                };

                let a: f32 = voice.clip.samples[index * clip_channels + source];
                let b: f32 = voice.clip.samples[next * clip_channels + source];
                *sample += (a + (b - a) * t) * gain;
            }

            if !voice.step() {
                return false;
            }
        }

        true
    });
}

/// Everything `cpal`'s output callback owns.
struct Mixer {
    commands: Consumer<MixerCommand>,
    volumes: Arc<SharedVolumes>,
    voices: Vec<Voice>,
    scratch: Vec<f32>,
    sample_rate: u32,
    channels: usize,
}

impl Mixer {
    /// Applies the queued commands and fills one output buffer, clamping the
    /// mix into `-1.0..=1.0` before converting it to the device's format.
    fn render<T>(&mut self, out: &mut [T])
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        while let Ok(command) = self.commands.pop() {
            apply_command(&mut self.voices, command, self.sample_rate);
        }

        self.scratch.resize(out.len(), 0.0);
        mix_into(&mut self.voices, self.volumes.load(), self.channels, &mut self.scratch);

        for (sample, mixed) in out.iter_mut().zip(&self.scratch) {
            *sample = T::from_sample(mixed.clamp(-1.0, 1.0));
        }
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut mixer: Mixer,
) -> Result<cpal::Stream, cpal::Error>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    device.build_output_stream(
        config,
        move |out: &mut [T], _info: &cpal::OutputCallbackInfo| mixer.render(out),
        |error: cpal::Error| log::error!("Audio stream error: {error}"),
        None,
    )
}

fn device_error(error: cpal::Error) -> RedixelError {
    RedixelError::AudioDevice(error.to_string())
}

/// The native [`AudioManager`], playing through the default output device.
pub struct NativeAudioManager {
    stream: cpal::Stream,
    commands: Producer<MixerCommand>,
    volumes: Arc<SharedVolumes>,
    decode_requests: Sender<DecodeRequest>,
    decode_results: Receiver<DecodeResult>,
    clips: ClipRegistry<Arc<DecodedClip>>,
    pending: PendingPlays,
}

impl NativeAudioManager {
    /// Opens the default output device and starts its stream at `config`'s
    /// volumes.
    ///
    /// Fails when there is no output device, its default stream cannot be
    /// built or started, or it only offers a non-PCM sample format.
    pub fn new(config: &AudioConfig) -> Result<Self, RedixelError> {
        let device: cpal::Device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| RedixelError::AudioDevice(String::from("no default output device")))?;
        let supported: cpal::SupportedStreamConfig = device.default_output_config().map_err(device_error)?;
        let stream_config: cpal::StreamConfig = supported.config();
        let channels: usize = usize::from(stream_config.channels);

        let (commands, command_queue): (Producer<MixerCommand>, Consumer<MixerCommand>) =
            RingBuffer::new(COMMAND_QUEUE_CAPACITY);
        let volumes: Arc<SharedVolumes> = Arc::new(SharedVolumes::new(config.clamped()));

        let mixer: Mixer = Mixer {
            commands: command_queue,
            volumes: Arc::clone(&volumes),
            voices: Vec::with_capacity(MAX_SFX_VOICES + MAX_MUSIC_VOICES),
            scratch: Vec::with_capacity(SCRATCH_FRAMES * channels),
            sample_rate: stream_config.sample_rate,
            channels,
        };

        let stream: cpal::Stream = match supported.sample_format() {
            cpal::SampleFormat::I8 => build_stream::<i8>(&device, stream_config, mixer),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, stream_config, mixer),
            cpal::SampleFormat::I24 => build_stream::<cpal::I24>(&device, stream_config, mixer),
            cpal::SampleFormat::I32 => build_stream::<i32>(&device, stream_config, mixer),
            cpal::SampleFormat::I64 => build_stream::<i64>(&device, stream_config, mixer),
            cpal::SampleFormat::U8 => build_stream::<u8>(&device, stream_config, mixer),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, stream_config, mixer),
            cpal::SampleFormat::U24 => build_stream::<cpal::U24>(&device, stream_config, mixer),
            cpal::SampleFormat::U32 => build_stream::<u32>(&device, stream_config, mixer),
            cpal::SampleFormat::U64 => build_stream::<u64>(&device, stream_config, mixer),
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, stream_config, mixer),
            cpal::SampleFormat::F64 => build_stream::<f64>(&device, stream_config, mixer),
            unsupported => {
                return Err(RedixelError::AudioDevice(format!(
                    "unsupported output sample format {unsupported:?}"
                )));
            }
        }
        .map_err(device_error)?;

        stream.play().map_err(device_error)?;

        let (decode_requests, decode_results): (Sender<DecodeRequest>, Receiver<DecodeResult>) =
            spawn_decoder(stream_config.sample_rate)?;

        Ok(Self {
            stream,
            commands,
            volumes,
            decode_requests,
            decode_results,
            clips: ClipRegistry::default(),
            pending: PendingPlays::default(),
        })
    }

    fn send(&mut self, command: MixerCommand) {
        if self.commands.push(command).is_err() {
            log::warn!("The audio command queue is full; a play or stop was dropped.");
        }
    }

    fn start_sound(&mut self, clip: SoundId, volume: f32, pitch: f32) {
        if let Some(decoded) = self.clips.get(clip) {
            let command: MixerCommand = MixerCommand::PlaySound {
                clip: Arc::clone(decoded),
                volume,
                pitch,
            };
            self.send(command);
        }
    }

    fn start_music(&mut self, clip: SoundId, options: MusicOptions) {
        if let Some(decoded) = self.clips.get(clip) {
            let command: MixerCommand = MixerCommand::PlayMusic {
                clip: Arc::clone(decoded),
                options,
            };
            self.send(command);
        }
    }
}

impl AudioManager for NativeAudioManager {
    fn load_clip(&mut self, id: SoundId, bytes: Vec<u8>) {
        self.clips.begin(id);

        if self.decode_requests.send((id, bytes)).is_err() {
            log::warn!("The audio decode thread has stopped; sound {} will play silently.", id.index());
            self.clips.finish(id, None);
        }
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
        self.send(MixerCommand::StopMusic {
            fade_out: clamp_fade(fade_out),
        });
    }

    fn set_master_volume(&mut self, volume: f32) {
        self.volumes.store_master(clamp_volume(volume));
    }

    fn set_channel_volume(&mut self, channel: AudioChannel, volume: f32) {
        self.volumes.store_channel(channel, clamp_volume(volume));
    }

    fn master_volume(&self) -> f32 {
        self.volumes.load().master_volume
    }

    fn channel_volume(&self, channel: AudioChannel) -> f32 {
        self.volumes.load().channel(channel)
    }

    fn suspend(&mut self) {
        if let Err(e) = self.stream.pause() {
            log::warn!("Failed to pause the audio stream: {e}");
        }
    }

    fn resume(&mut self) {
        if let Err(e) = self.stream.play() {
            log::warn!("Failed to resume the audio stream: {e}");
        }
    }

    fn update(&mut self, dt: f32) {
        while let Ok((id, result)) = self.decode_results.try_recv() {
            let clip: Option<Arc<DecodedClip>> = match result {
                Ok(decoded) => Some(Arc::new(decoded)),
                Err(e) => {
                    log::warn!("Failed to decode sound {}: {e}. It will play silently.", id.index());
                    None
                }
            };
            self.clips.finish(id, clip);
        }

        let clips: &ClipRegistry<Arc<DecodedClip>> = &self.clips;
        let ready: Vec<ReadyPlay> = self.pending.take_ready(dt, |id: SoundId| clips.state(id));

        for play in ready {
            match play {
                ReadyPlay::Sound { clip, volume, pitch } => self.start_sound(clip, volume, pitch),
                ReadyPlay::Music { clip, options } => self.start_music(clip, options),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 48000;

    fn make_wav(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_size: u32 = (samples.len() * 2) as u32;
        let mut wav: Vec<u8> = Vec::new();

        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());

        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }

        wav
    }

    fn clip(channels: usize, samples: &[f32]) -> Arc<DecodedClip> {
        Arc::new(DecodedClip {
            samples: samples.into(),
            channels,
        })
    }

    fn sound(clip: Arc<DecodedClip>, pitch: f32) -> MixerCommand {
        MixerCommand::PlaySound {
            clip,
            volume: 1.0,
            pitch,
        }
    }

    fn music(clip: Arc<DecodedClip>, fade_out_previous: f32) -> MixerCommand {
        MixerCommand::PlayMusic {
            clip,
            options: MusicOptions {
                fade_out_previous,
                ..MusicOptions::default()
            },
        }
    }

    fn music_voices(voices: &[Voice]) -> usize {
        voices
            .iter()
            .filter(|voice: &&Voice| voice.channel == AudioChannel::Music)
            .count()
    }

    #[test]
    fn decode_clip_keeps_every_packet_of_a_multi_packet_file() {
        let samples: Vec<i16> = (0..5000).map(|i: i32| ((i % 2000) - 1000) as i16 * 16).collect();

        let decoded: DecodedClip = decode_clip(make_wav(44100, &samples), 44100).expect("a valid WAV decodes");

        assert_eq!(
            decoded.samples.len(),
            samples.len(),
            "copy_to_vec_interleaved replaces its target, so every packet has to be appended"
        );
        assert_eq!(decoded.channels, 1);
    }

    #[test]
    fn decode_clip_resamples_to_the_output_rate() {
        let samples: Vec<i16> = vec![1000; 22050];

        let decoded: DecodedClip = decode_clip(make_wav(22050, &samples), 44100).expect("a valid WAV decodes");

        assert_eq!(decoded.frames(), 44100);
    }

    #[test]
    fn decode_clip_rejects_bytes_that_are_not_audio() {
        assert!(matches!(
            decode_clip(vec![0, 1, 2, 3], 44100),
            Err(RedixelError::AudioDecode(..))
        ));
    }

    #[test]
    fn resample_linear_is_a_passthrough_when_rates_match() {
        let input: Vec<f32> = vec![0.1, -0.2, 0.3, -0.4];
        assert_eq!(resample_linear(&input, 2, 44100, 44100), input);
    }

    #[test]
    fn resample_linear_halves_the_frame_count_when_downsampling_by_half() {
        let input: Vec<f32> = (0..200).map(|i: i32| i as f32).collect();
        assert_eq!(resample_linear(&input, 2, 44100, 22050).len(), 100);
    }

    #[test]
    fn a_mono_clip_plays_on_every_output_channel() {
        let mut voices: Vec<Voice> = Vec::new();
        apply_command(&mut voices, sound(clip(1, &[0.5, 0.5]), 1.0), SAMPLE_RATE);

        let mut out: Vec<f32> = vec![0.0; 2];
        mix_into(&mut voices, AudioConfig::default(), 2, &mut out);

        assert_eq!(out, [0.5, 0.5]);
    }

    #[test]
    fn output_channels_a_clip_has_no_source_for_stay_silent() {
        let mut voices: Vec<Voice> = Vec::new();
        apply_command(&mut voices, sound(clip(2, &[0.25, 0.75, 0.25, 0.75]), 1.0), SAMPLE_RATE);

        let mut out: Vec<f32> = vec![0.0; 4];
        mix_into(&mut voices, AudioConfig::default(), 4, &mut out);

        assert_eq!(out, [0.25, 0.75, 0.0, 0.0]);
    }

    #[test]
    fn a_sound_is_removed_once_it_reaches_its_end() {
        let mut voices: Vec<Voice> = Vec::new();
        apply_command(&mut voices, sound(clip(1, &[1.0, 1.0, 1.0]), 1.0), SAMPLE_RATE);

        let mut out: Vec<f32> = vec![0.0; 4];
        mix_into(&mut voices, AudioConfig::default(), 1, &mut out);

        assert_eq!(out, [1.0, 1.0, 1.0, 0.0]);
        assert!(voices.is_empty());
    }

    #[test]
    fn a_higher_pitch_reaches_the_end_sooner() {
        let mut voices: Vec<Voice> = Vec::new();
        apply_command(&mut voices, sound(clip(1, &[1.0; 8]), 2.0), SAMPLE_RATE);

        let mut out: Vec<f32> = vec![0.0; 8];
        mix_into(&mut voices, AudioConfig::default(), 1, &mut out);

        assert_eq!(out.iter().filter(|sample: &&f32| **sample > 0.0).count(), 4);
        assert!(voices.is_empty());
    }

    #[test]
    fn looped_music_wraps_around_and_keeps_playing() {
        let mut voices: Vec<Voice> = Vec::new();
        apply_command(&mut voices, music(clip(1, &[0.1, 0.2]), 0.0), SAMPLE_RATE);

        let mut out: Vec<f32> = vec![0.0; 5];
        mix_into(&mut voices, AudioConfig::default(), 1, &mut out);

        assert_eq!(out, [0.1, 0.2, 0.1, 0.2, 0.1]);
        assert_eq!(voices.len(), 1);
    }

    #[test]
    fn master_and_channel_volume_scale_the_mix() {
        let mut voices: Vec<Voice> = Vec::new();
        apply_command(&mut voices, sound(clip(1, &[1.0, 1.0]), 1.0), SAMPLE_RATE);

        let volumes: AudioConfig = AudioConfig {
            master_volume: 0.5,
            sfx_volume: 0.5,
            music_volume: 1.0,
        };
        let mut out: Vec<f32> = vec![0.0; 1];
        mix_into(&mut voices, volumes, 1, &mut out);

        assert_eq!(out, [0.25]);
    }

    #[test]
    fn switching_tracks_never_leaves_more_than_two_music_voices() {
        let mut voices: Vec<Voice> = Vec::new();
        for _ in 0..5 {
            apply_command(&mut voices, music(clip(1, &[0.1; 16]), 10.0), SAMPLE_RATE);
        }

        assert_eq!(music_voices(&voices), MAX_MUSIC_VOICES);
        assert!(!voices.last().expect("the newest track is playing").is_stopping());
    }

    #[test]
    fn stopping_music_fades_it_out_then_removes_it() {
        let mut voices: Vec<Voice> = Vec::new();
        apply_command(&mut voices, music(clip(1, &[1.0; 4]), 0.0), SAMPLE_RATE);
        apply_command(&mut voices, MixerCommand::StopMusic { fade_out: 0.0 }, SAMPLE_RATE);

        let fade_frames: usize = (MIN_FADE_SECS * SAMPLE_RATE as f32).round() as usize;
        let mut out: Vec<f32> = vec![0.0; fade_frames + 1];
        mix_into(&mut voices, AudioConfig::default(), 1, &mut out);

        assert_eq!(out[0], 1.0);
        assert!(out[fade_frames - 1] < 0.01, "the fade must end near silence, not cut");
        assert_eq!(out[fade_frames], 0.0);
        assert!(voices.is_empty());
    }

    #[test]
    fn sound_effects_beyond_the_voice_cap_are_dropped() {
        let mut voices: Vec<Voice> = Vec::new();
        for _ in 0..MAX_SFX_VOICES + 8 {
            apply_command(&mut voices, sound(clip(1, &[1.0]), 1.0), SAMPLE_RATE);
        }

        assert_eq!(voices.len(), MAX_SFX_VOICES);
    }

    #[test]
    fn render_clamps_an_overdriven_mix() {
        let (mut producer, consumer): (Producer<MixerCommand>, Consumer<MixerCommand>) = RingBuffer::new(4);
        let mut mixer: Mixer = Mixer {
            commands: consumer,
            volumes: Arc::new(SharedVolumes::new(AudioConfig::default())),
            voices: Vec::new(),
            scratch: Vec::new(),
            sample_rate: SAMPLE_RATE,
            channels: 1,
        };

        for _ in 0..3 {
            assert!(producer.push(sound(clip(1, &[0.9, 0.9]), 1.0)).is_ok());
        }

        let mut out: Vec<i16> = vec![0; 1];
        mixer.render(&mut out);

        assert_eq!(out, [i16::MAX]);
    }
}
