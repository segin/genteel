//! Audio Output Module
//!
//! Provides cross-platform audio output using cpal.
//!
//! Samples travel from the emulation thread to the audio callback through
//! [`AudioBuffer`], a lock-free single-producer/single-consumer ring of packed
//! stereo frames: the callback never blocks on, or skips because of, the
//! producer. The consumer side ([`AudioConsumer`]) bounds playback latency
//! with dynamic rate control: it resamples by at most ±0.5 % to keep the
//! ring near a target fill level, so host/emulator clock drift neither grows
//! the latency nor forces samples to be discarded in normal operation.

#[cfg(feature = "gui")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// Sample rate for audio output (Native Genesis FM rate: 53693175 / 7 / 144 = ~53267)
pub const SAMPLE_RATE: u32 = 53267;

/// Genesis Master Clock (NTSC)
pub const NTSC_MCLK: u32 = 53693175;
/// Genesis Master Clock (PAL)
pub const PAL_MCLK: u32 = 53203424;

/// Target frames per second
pub const FPS: u32 = 60;
pub const PAL_FPS: u32 = 50;

/// Audio buffer size (in stereo sample pairs)
pub const BUFFER_SIZE: usize = 512;

/// Fill level the consumer steers the ring towards.
pub const TARGET_LATENCY_MS: u32 = 40;
/// Hard upper bound on buffered audio; beyond it the consumer skips ahead.
pub const MAX_LATENCY_MS: u32 = 100;
/// Largest resampling-ratio deviation used by the dynamic rate control.
pub const MAX_RATE_ADJUST: f64 = 0.005;

fn pack_stereo(left: i16, right: i16) -> u32 {
    ((left as u16 as u32) << 16) | right as u16 as u32
}

fn unpack_stereo(packed: u32) -> (i16, i16) {
    (((packed >> 16) as u16) as i16, (packed as u16) as i16)
}

fn frames_for_ms(sample_rate: u32, ms: u32) -> usize {
    (sample_rate as u64 * ms as u64 / 1000) as usize
}

/// Lock-free SPSC ring of stereo frames shared between the emulation thread
/// (producer) and the audio callback (consumer).
///
/// `push` must only be called from one producer thread and the `pop*`/
/// `clear` family only from one consumer thread at a time.
#[derive(Debug)]
pub struct AudioBuffer {
    /// Packed stereo frames (`left << 16 | right`).
    slots: Box<[AtomicU32]>,
    /// `slots.len() - 1`; the capacity is a power of two.
    mask: usize,
    /// Total frames ever consumed (owned by the consumer).
    read: AtomicUsize,
    /// Total frames ever produced (owned by the producer).
    write: AtomicUsize,
    /// Producer-only: a left sample awaiting its right partner
    /// (`1 << 16 | sample`), or 0.
    pending_left: AtomicU32,
    /// Frames the consumer steers towards.
    target_frames: AtomicUsize,
    /// Frames beyond which the consumer skips ahead.
    max_frames: AtomicUsize,
    /// Frames rejected because the ring was full.
    overflow_frames: AtomicU64,
    /// Frames skipped by the consumer to enforce the latency bound.
    skipped_frames: AtomicU64,
}

impl AudioBuffer {
    /// Create a new audio buffer holding at least `capacity` stereo frames.
    pub fn new(capacity: usize) -> Self {
        let size = capacity.max(2).next_power_of_two();
        let slots: Vec<AtomicU32> = (0..size).map(|_| AtomicU32::new(0)).collect();
        let max = frames_for_ms(SAMPLE_RATE, MAX_LATENCY_MS).min(size);
        let target = frames_for_ms(SAMPLE_RATE, TARGET_LATENCY_MS).min(max / 2);
        Self {
            slots: slots.into_boxed_slice(),
            mask: size - 1,
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
            pending_left: AtomicU32::new(0),
            target_frames: AtomicUsize::new(target),
            max_frames: AtomicUsize::new(max),
            overflow_frames: AtomicU64::new(0),
            skipped_frames: AtomicU64::new(0),
        }
    }

    /// Capacity in stereo frames.
    pub fn capacity_frames(&self) -> usize {
        self.slots.len()
    }

    /// Configure the latency window for a device running at `sample_rate`.
    pub fn set_latency(&self, sample_rate: u32, target_ms: u32, max_ms: u32) {
        let max = frames_for_ms(sample_rate, max_ms.max(1)).clamp(2, self.slots.len());
        let target = frames_for_ms(sample_rate, target_ms).clamp(1, max / 2);
        self.max_frames.store(max, Ordering::Relaxed);
        self.target_frames.store(target, Ordering::Relaxed);
    }

    /// Fill level (frames) the consumer steers towards.
    pub fn target_frames(&self) -> usize {
        self.target_frames.load(Ordering::Relaxed)
    }

    /// Maximum buffered frames before the consumer skips ahead.
    pub fn max_frames(&self) -> usize {
        self.max_frames.load(Ordering::Relaxed)
    }

    /// Frames rejected by `push` because the ring was full.
    pub fn overflow_frames(&self) -> u64 {
        self.overflow_frames.load(Ordering::Relaxed)
    }

    /// Frames skipped by the consumer to enforce the latency bound.
    pub fn skipped_frames(&self) -> u64 {
        self.skipped_frames.load(Ordering::Relaxed)
    }

    /// Buffered stereo frames.
    pub fn available_frames(&self) -> usize {
        let w = self.write.load(Ordering::Acquire);
        let r = self.read.load(Ordering::Acquire);
        w.wrapping_sub(r).min(self.slots.len())
    }

    /// Get number of available samples (two per stereo frame).
    pub fn available(&self) -> usize {
        self.available_frames() * 2
    }

    /// Push interleaved stereo samples (producer side). A trailing odd sample
    /// is held until its right partner arrives. Frames that do not fit are
    /// dropped and counted; the consumer's rate control keeps this from
    /// happening in normal operation.
    pub fn push(&self, samples: &[i16]) {
        let mut iter = samples.iter().copied();
        let pending = self.pending_left.swap(0, Ordering::Relaxed);
        let mut w = self.write.load(Ordering::Relaxed);
        let r = self.read.load(Ordering::Acquire);
        let mut free = self.slots.len() - w.wrapping_sub(r).min(self.slots.len());
        let mut dropped = 0u64;
        let mut put = |w: &mut usize, free: &mut usize, frame: u32| {
            if *free == 0 {
                dropped += 1;
                return;
            }
            self.slots[*w & self.mask].store(frame, Ordering::Relaxed);
            *w = w.wrapping_add(1);
            *free -= 1;
        };
        if pending != 0 {
            match iter.next() {
                Some(right) => put(&mut w, &mut free, pack_stereo(pending as u16 as i16, right)),
                None => {
                    self.pending_left.store(pending, Ordering::Relaxed);
                    return;
                }
            }
        }
        loop {
            match (iter.next(), iter.next()) {
                (Some(left), Some(right)) => put(&mut w, &mut free, pack_stereo(left, right)),
                (Some(left), None) => {
                    self.pending_left
                        .store((1 << 16) | left as u16 as u32, Ordering::Relaxed);
                    break;
                }
                _ => break,
            }
        }
        self.write.store(w, Ordering::Release);
        if dropped > 0 {
            self.overflow_frames.fetch_add(dropped, Ordering::Relaxed);
        }
    }

    /// Pop one stereo frame (consumer side).
    pub fn pop_frame(&self) -> Option<(i16, i16)> {
        let r = self.read.load(Ordering::Relaxed);
        let w = self.write.load(Ordering::Acquire);
        if w == r {
            return None;
        }
        let frame = self.slots[r & self.mask].load(Ordering::Relaxed);
        self.read.store(r.wrapping_add(1), Ordering::Release);
        Some(unpack_stereo(frame))
    }

    /// Discard up to `frames` of the oldest buffered frames (consumer side).
    fn skip_frames(&self, frames: usize) {
        let frames = frames.min(self.available_frames());
        if frames == 0 {
            return;
        }
        let r = self.read.load(Ordering::Relaxed);
        self.read.store(r.wrapping_add(frames), Ordering::Release);
        self.skipped_frames
            .fetch_add(frames as u64, Ordering::Relaxed);
    }

    /// Pop interleaved samples into `dest` (consumer side); underruns and a
    /// trailing odd slot are filled with silence.
    pub fn pop(&self, dest: &mut [i16]) {
        for pair in dest.chunks_mut(2) {
            let (l, r) = self.pop_frame().unwrap_or((0, 0));
            pair[0] = l;
            if pair.len() > 1 {
                pair[1] = r;
            } else {
                pair[0] = 0;
            }
        }
    }

    /// Pop samples as f32 in [-1.0, 1.0) (consumer side).
    pub fn pop_f32(&self, dest: &mut [f32]) {
        for pair in dest.chunks_mut(2) {
            let (l, r) = self.pop_frame().unwrap_or((0, 0));
            pair[0] = l as f32 / 32768.0;
            if pair.len() > 1 {
                pair[1] = r as f32 / 32768.0;
            } else {
                pair[0] = 0.0;
            }
        }
    }

    /// Drop all buffered frames (consumer side, or while no producer runs).
    pub fn clear(&self) {
        let w = self.write.load(Ordering::Acquire);
        self.read.store(w, Ordering::Release);
    }
}

/// Producer handle returned by [`SharedAudioBuffer::try_lock`].
pub struct AudioProducer<'a>(&'a AudioBuffer);

impl AudioProducer<'_> {
    /// Push interleaved stereo samples.
    pub fn push(&mut self, samples: &[i16]) {
        self.0.push(samples);
    }

    /// Get number of available samples.
    pub fn available(&self) -> usize {
        self.0.available()
    }
}

/// Error type of [`SharedAudioBuffer::try_lock`]; the lock-free ring never
/// produces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioBufferBusy;

/// Shared handle to the lock-free audio ring.
#[derive(Debug, Clone)]
pub struct SharedAudioBuffer(Arc<AudioBuffer>);

impl SharedAudioBuffer {
    /// Access the producer side. The ring is lock-free, so this always
    /// succeeds; it keeps the call shape of the former mutex-based buffer.
    pub fn try_lock(&self) -> Result<AudioProducer<'_>, AudioBufferBusy> {
        Ok(AudioProducer(&self.0))
    }

    /// Push interleaved stereo samples (producer side).
    pub fn push(&self, samples: &[i16]) {
        self.0.push(samples);
    }

    /// The underlying ring.
    pub fn ring(&self) -> &AudioBuffer {
        &self.0
    }
}

impl std::ops::Deref for SharedAudioBuffer {
    type Target = AudioBuffer;

    fn deref(&self) -> &AudioBuffer {
        &self.0
    }
}

/// Create a new shared audio buffer
pub fn create_audio_buffer() -> SharedAudioBuffer {
    SharedAudioBuffer(Arc::new(AudioBuffer::new(BUFFER_SIZE * 64)))
}

/// Consumer side of the audio ring: dynamic rate control, latency bound and
/// underrun handling for the audio callback.
#[derive(Debug)]
pub struct AudioConsumer {
    buffer: Arc<AudioBuffer>,
    /// Interpolation position between `prev` and `next`, in [0, 1).
    pos: f64,
    prev: (f32, f32),
    next: (f32, f32),
    /// Smoothed fill level in frames.
    fill_avg: f64,
    /// Output is held silent until the ring first reaches the target.
    primed: bool,
    /// Ratio of input frames consumed per output frame.
    ratio: f64,
}

impl AudioConsumer {
    pub fn new(buffer: SharedAudioBuffer) -> Self {
        Self {
            buffer: buffer.0,
            pos: 0.0,
            prev: (0.0, 0.0),
            next: (0.0, 0.0),
            fill_avg: 0.0,
            primed: false,
            ratio: 1.0,
        }
    }

    /// Current resampling ratio (input frames per output frame).
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Call once per audio callback before producing frames: enforces the
    /// latency bound and updates the rate control from the fill level.
    pub fn begin_callback(&mut self) {
        let fill = self.buffer.available_frames();
        let target = self.buffer.target_frames().max(1);
        let max = self.buffer.max_frames();
        if fill > max {
            /* Abnormal backlog (e.g. a producer burst after a stall):
             * skip back to the target instead of playing it late. */
            self.buffer.skip_frames(fill - target);
            self.fill_avg = target as f64;
        }
        let fill = self.buffer.available_frames();
        if !self.primed {
            if fill >= target {
                self.primed = true;
                self.fill_avg = fill as f64;
            }
            self.ratio = 1.0;
            return;
        }
        self.fill_avg += (fill as f64 - self.fill_avg) * 0.1;
        let error = ((self.fill_avg - target as f64) / target as f64).clamp(-1.0, 1.0);
        self.ratio = 1.0 + MAX_RATE_ADJUST * error;
    }

    fn decay(v: f32) -> f32 {
        let d = v * (31.0 / 32.0);
        if d.abs() < 1.0 {
            0.0
        } else {
            d
        }
    }

    fn advance_input(&mut self) {
        self.prev = self.next;
        self.next = match self.buffer.pop_frame() {
            Some((l, r)) => (l as f32, r as f32),
            None => {
                /* Underrun: hold the last frame, decaying toward zero so a
                 * stopped producer does not park DC on the output, and
                 * re-prime before resuming. */
                self.primed = false;
                (Self::decay(self.next.0), Self::decay(self.next.1))
            }
        };
    }

    /// Produce the next output frame.
    pub fn next_frame(&mut self) -> (i16, i16) {
        if !self.primed {
            self.prev = (Self::decay(self.prev.0), Self::decay(self.prev.1));
            self.next = self.prev;
            return (self.prev.0 as i16, self.prev.1 as i16);
        }
        self.pos += self.ratio;
        while self.pos >= 1.0 {
            self.pos -= 1.0;
            self.advance_input();
        }
        let t = self.pos as f32;
        let l = self.prev.0 + (self.next.0 - self.prev.0) * t;
        let r = self.prev.1 + (self.next.1 - self.prev.1) * t;
        (
            l.round().clamp(-32768.0, 32767.0) as i16,
            r.round().clamp(-32768.0, 32767.0) as i16,
        )
    }
}

/// Audio output stream wrapper
#[cfg(feature = "gui")]
pub struct AudioOutput {
    _stream: cpal::Stream,
    pub sample_rate: u32,
}

#[cfg(feature = "gui")]
impl AudioOutput {
    fn write_output_f32(data: &mut [f32], channels: usize, consumer: &mut AudioConsumer) {
        if channels == 0 {
            /* chunks_mut(0) panics; a device reporting no output channels
             * gets silence by doing nothing. */
            return;
        }
        consumer.begin_callback();
        for frame in data.chunks_mut(channels) {
            let (left, right) = consumer.next_frame();
            match channels {
                1 => frame[0] = ((left as i32 + right as i32) as f32 / 2.0) / 32768.0,
                _ => {
                    frame[0] = left as f32 / 32768.0;
                    frame[1] = right as f32 / 32768.0;
                    for sample in &mut frame[2..] {
                        *sample = 0.0;
                    }
                }
            }
        }
    }

    fn write_output_i16(data: &mut [i16], channels: usize, consumer: &mut AudioConsumer) {
        if channels == 0 {
            return;
        }
        consumer.begin_callback();
        for frame in data.chunks_mut(channels) {
            let (left, right) = consumer.next_frame();
            match channels {
                1 => frame[0] = ((left as i32 + right as i32) / 2) as i16,
                _ => {
                    frame[0] = left;
                    frame[1] = right;
                    for sample in &mut frame[2..] {
                        *sample = 0;
                    }
                }
            }
        }
    }

    fn write_output_u16(data: &mut [u16], channels: usize, consumer: &mut AudioConsumer) {
        if channels == 0 {
            return;
        }
        consumer.begin_callback();
        for frame in data.chunks_mut(channels) {
            let (left, right) = consumer.next_frame();
            match channels {
                1 => frame[0] = (((left as i32 + right as i32) / 2) + 32768) as u16,
                _ => {
                    frame[0] = (left as i32 + 32768) as u16;
                    frame[1] = (right as i32 + 32768) as u16;
                    for sample in &mut frame[2..] {
                        *sample = u16::MAX / 2;
                    }
                }
            }
        }
    }

    /// Pick a stream config this code can actually render to (F32/I16/U16
    /// with at least one channel), preferring the device default.
    fn usable_config(device: &cpal::Device) -> Option<cpal::SupportedStreamConfig> {
        let renderable = |config: &cpal::SupportedStreamConfig| {
            config.channels() > 0
                && matches!(
                    config.sample_format(),
                    cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
                )
        };
        if let Ok(config) = device.default_output_config() {
            if renderable(&config) {
                return Some(config);
            }
        }
        // The default format isn't one we render; look for any supported
        // range with a renderable format instead of erroring out.
        if let Ok(ranges) = device.supported_output_configs() {
            for range in ranges {
                let config = range.with_max_sample_rate();
                if renderable(&config) {
                    return Some(config);
                }
            }
        }
        None
    }

    /// Create a new audio output using cpal directly
    pub fn new(buffer: SharedAudioBuffer) -> Result<Self, String> {
        let host = cpal::default_host();
        let mut selected = None;

        if let Some(device) = host.default_output_device() {
            if let Some(config) = Self::usable_config(&device) {
                selected = Some((device, config));
            }
        }

        if selected.is_none() {
            if let Ok(mut devices) = host.output_devices() {
                selected = devices
                    .find_map(|device| Self::usable_config(&device).map(|config| (device, config)));
            }
        }

        let (device, supported_config) =
            selected.ok_or_else(|| "Failed to locate a usable audio output device".to_string())?;
        let sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels() as usize;
        let sample_format = supported_config.sample_format();
        let stream_config: cpal::StreamConfig = supported_config.into();
        let err_fn = |err| eprintln!("Audio stream error: {}", err);
        buffer.set_latency(sample_rate, TARGET_LATENCY_MS, MAX_LATENCY_MS);
        let mut consumer = AudioConsumer::new(buffer);

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &stream_config,
                move |data: &mut [f32], _| Self::write_output_f32(data, channels, &mut consumer),
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &stream_config,
                move |data: &mut [i16], _| Self::write_output_i16(data, channels, &mut consumer),
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_output_stream(
                &stream_config,
                move |data: &mut [u16], _| Self::write_output_u16(data, channels, &mut consumer),
                err_fn,
                None,
            ),
            other => {
                return Err(format!("Unsupported audio sample format: {:?}", other));
            }
        }
        .map_err(|e| format!("Failed to build audio output stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start audio output stream: {}", e))?;

        Ok(Self {
            _stream: stream,
            sample_rate,
        })
    }
}

/// Calculate samples needed per frame
/// Genesis runs at ~60fps NTSC, so samples_per_frame = sample_rate / 60
pub fn samples_per_frame() -> usize {
    samples_per_frame_for_rate_and_region(SAMPLE_RATE, false)
}

pub fn samples_per_frame_for_rate(sample_rate: u32) -> usize {
    samples_per_frame_for_rate_and_region(sample_rate, false)
}

pub fn samples_per_frame_for_rate_and_region(sample_rate: u32, is_pal: bool) -> usize {
    let fps = if is_pal { PAL_FPS } else { FPS };
    (sample_rate as f32 / fps as f32).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_pack_unpack_stereo_round_trip() {
        let packed = pack_stereo(-1234, 5678);
        assert_eq!(unpack_stereo(packed), (-1234, 5678));
    }

    #[test]
    fn test_create_audio_buffer() {
        let shared = create_audio_buffer();
        assert_eq!(shared.capacity_frames(), BUFFER_SIZE * 64);
        assert_eq!(shared.available(), 0);
    }

    #[test]
    fn test_try_lock_always_grants_producer_access() {
        let shared = create_audio_buffer();
        let other = shared.clone();
        {
            let mut producer = shared.try_lock().expect("lock-free ring never blocks");
            producer.push(&[1, 2, 3, 4]);
            assert_eq!(producer.available(), 4);
        }
        assert_eq!(other.available(), 4);
        assert_eq!(other.pop_frame(), Some((1, 2)));
    }

    #[test]
    fn test_audio_buffer_push_pop() {
        let buf = AudioBuffer::new(64);
        buf.push(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(buf.available(), 6);
        let mut out = [0i16; 6];
        buf.pop(&mut out);
        assert_eq!(out, [1, 2, 3, 4, 5, 6]);
        assert_eq!(buf.available(), 0);
    }

    #[test]
    fn test_audio_buffer_underrun_fills_silence() {
        let buf = AudioBuffer::new(64);
        buf.push(&[7, 8]);
        let mut out = [9i16; 6];
        buf.pop(&mut out);
        assert_eq!(out, [7, 8, 0, 0, 0, 0]);
    }

    #[test]
    fn test_odd_sample_waits_for_partner() {
        let buf = AudioBuffer::new(64);
        buf.push(&[10, 20, 30]);
        assert_eq!(buf.available(), 2);
        buf.push(&[40]);
        assert_eq!(buf.available(), 4);
        assert_eq!(buf.pop_frame(), Some((10, 20)));
        assert_eq!(buf.pop_frame(), Some((30, 40)));
    }

    #[test]
    fn test_full_ring_rejects_newest_and_counts() {
        let buf = AudioBuffer::new(4);
        buf.push(&[1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
        assert_eq!(buf.available_frames(), 4);
        assert_eq!(buf.overflow_frames(), 1);
        assert_eq!(buf.pop_frame(), Some((1, 1)));
    }

    #[test]
    fn test_wrap_around() {
        let buf = AudioBuffer::new(4);
        for round in 0..10i16 {
            buf.push(&[round, -round, round + 1, -round - 1]);
            assert_eq!(buf.pop_frame(), Some((round, -round)));
            assert_eq!(buf.pop_frame(), Some((round + 1, -round - 1)));
        }
        assert_eq!(buf.pop_frame(), None);
    }

    #[test]
    fn test_pop_f32_scaling() {
        let buf = AudioBuffer::new(8);
        buf.push(&[i16::MIN, i16::MAX, 0, 16384]);
        let mut out = [1.0f32; 6];
        buf.pop_f32(&mut out);
        assert_eq!(out[0], -1.0);
        assert!((out[1] - 32767.0 / 32768.0).abs() < 1e-6);
        assert_eq!(out[2], 0.0);
        assert_eq!(out[3], 0.5);
        assert_eq!(&out[4..], &[0.0, 0.0]);
    }

    #[test]
    fn test_clear_drops_buffered_frames() {
        let buf = AudioBuffer::new(8);
        buf.push(&[1, 2, 3, 4]);
        buf.clear();
        assert_eq!(buf.available(), 0);
        buf.push(&[5, 6]);
        assert_eq!(buf.pop_frame(), Some((5, 6)));
    }

    #[test]
    fn test_samples_per_frame() {
        assert_eq!(
            samples_per_frame(),
            (SAMPLE_RATE as f32 / 60.0).ceil() as usize
        );
        assert_eq!(samples_per_frame_for_rate(48_000), 800);
    }

    #[test]
    fn test_samples_per_frame_for_pal_rate() {
        assert_eq!(samples_per_frame_for_rate_and_region(48_000, true), 960);
    }

    #[test]
    fn test_set_latency_converts_milliseconds() {
        let buf = AudioBuffer::new(BUFFER_SIZE * 64);
        buf.set_latency(48_000, 40, 100);
        assert_eq!(buf.target_frames(), 1920);
        assert_eq!(buf.max_frames(), 4800);
    }

    #[test]
    fn test_consumer_holds_silence_until_primed() {
        let shared = create_audio_buffer();
        shared.set_latency(1000, 10, 40); // target 10 frames
        let mut consumer = AudioConsumer::new(shared.clone());
        shared.push(&[100, 100, 100, 100]);
        consumer.begin_callback();
        assert_eq!(consumer.next_frame(), (0, 0));
        shared.push(&[100; 20]);
        consumer.begin_callback();
        let mut last = (0, 0);
        for _ in 0..4 {
            last = consumer.next_frame();
        }
        assert_eq!(last, (100, 100));
    }

    #[test]
    fn test_consumer_decays_held_sample_on_underrun() {
        let shared = create_audio_buffer();
        shared.set_latency(1000, 2, 10);
        let mut consumer = AudioConsumer::new(shared.clone());
        shared.push(&[8000, -8000, 8000, -8000]);
        consumer.begin_callback();
        let mut seen = Vec::new();
        for _ in 0..400 {
            seen.push(consumer.next_frame());
        }
        assert!(seen.iter().any(|&(l, _)| l >= 7000));
        assert_eq!(*seen.last().unwrap(), (0, 0));
    }

    #[test]
    fn test_consumer_skips_backlog_beyond_latency_bound() {
        let shared = create_audio_buffer();
        shared.set_latency(1000, 10, 50);
        let mut consumer = AudioConsumer::new(shared.clone());
        shared.push(&vec![1i16; 2 * 200]);
        consumer.begin_callback();
        assert_eq!(shared.available_frames(), 10);
        assert_eq!(shared.skipped_frames(), 190);
    }

    /// Simulate a producer whose clock runs `drift` faster/slower than the
    /// consumer's and check the fill level settles inside the latency bound
    /// without skipping or overflowing.
    fn simulate_drift(drift: f64) -> (usize, u64, u64) {
        let rate = 48_000u32;
        let shared = create_audio_buffer();
        shared.set_latency(rate, TARGET_LATENCY_MS, MAX_LATENCY_MS);
        let mut consumer = AudioConsumer::new(shared.clone());
        let frame_samples = (rate as f64 / 60.0) * (1.0 + drift);
        let callback_frames = 512usize;
        let mut produced = 0.0f64;
        let mut consumed = 0usize;
        let mut max_fill = 0usize;
        /* Ten simulated minutes. */
        let total = rate as usize * 60 * 10;
        while consumed < total {
            /* Producer pushes whole emulated frames when it falls behind. */
            while produced < consumed as f64 + frame_samples {
                let n = frame_samples.round() as usize;
                shared.push(&vec![0i16; n * 2]);
                produced += frame_samples;
            }
            consumer.begin_callback();
            for _ in 0..callback_frames {
                consumer.next_frame();
            }
            consumed += callback_frames;
            if consumed > rate as usize * 60 {
                max_fill = max_fill.max(shared.available_frames());
            }
        }
        (max_fill, shared.skipped_frames(), shared.overflow_frames())
    }

    #[test]
    fn test_rate_control_bounds_latency_under_clock_drift() {
        for drift in [-0.003, -0.0013, 0.0, 0.0013, 0.003] {
            let (max_fill, skipped, overflow) = simulate_drift(drift);
            assert!(
                max_fill <= frames_for_ms(48_000, MAX_LATENCY_MS),
                "drift {drift}: fill {max_fill} exceeds bound"
            );
            assert_eq!(skipped, 0, "drift {drift}: samples were skipped");
            assert_eq!(overflow, 0, "drift {drift}: ring overflowed");
        }
    }

    #[test]
    fn test_spsc_threads_deliver_frames_in_order() {
        let shared = create_audio_buffer();
        let producer = shared.clone();
        let total = 200_000i32;
        let handle = std::thread::spawn(move || {
            let mut next = 0i32;
            while next < total {
                let chunk: Vec<i16> = (next..(next + 64).min(total))
                    .flat_map(|v| [(v & 0x7FFF) as i16, !(v & 0x7FFF) as i16])
                    .collect();
                while producer.available_frames() + 64 > producer.capacity_frames() {
                    std::thread::yield_now();
                }
                producer.push(&chunk);
                next += 64;
            }
        });
        let mut expect = 0i32;
        while expect < total {
            if let Some((l, r)) = shared.pop_frame() {
                assert_eq!(l, (expect & 0x7FFF) as i16);
                assert_eq!(r, !(expect & 0x7FFF) as i16);
                expect += 1;
            } else {
                std::thread::yield_now();
            }
        }
        handle.join().unwrap();
        assert_eq!(shared.overflow_frames(), 0);
    }

    proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
        #[test]
        fn prop_push_pop_preserves_frames(
            capacity in 2usize..512,
            chunks in prop::collection::vec(prop::collection::vec(any::<i16>(), 0..64), 0..32)
        ) {
            let buf = AudioBuffer::new(capacity);
            let mut model: std::collections::VecDeque<(i16, i16)> = Default::default();
            let mut carry: Option<i16> = None;
            for chunk in &chunks {
                buf.push(chunk);
                for &s in chunk {
                    match carry.take() {
                        Some(l) => {
                            if model.len() < buf.capacity_frames() {
                                model.push_back((l, s));
                            }
                        }
                        None => carry = Some(s),
                    }
                }
                /* Drain half of the model each step. */
                for _ in 0..model.len() / 2 {
                    prop_assert_eq!(buf.pop_frame(), model.pop_front());
                }
            }
            while let Some(frame) = model.pop_front() {
                prop_assert_eq!(buf.pop_frame(), Some(frame));
            }
            prop_assert_eq!(buf.pop_frame(), None);
        }
    }
}
