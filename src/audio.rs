//! Audio Output Module
//!
//! Provides cross-platform audio output using cpal.
//! Uses a ring buffer to transfer samples from emulation thread to audio callback.

#[cfg(feature = "gui")]
use cpal::traits::{DeviceTrait, HostTrait};
#[cfg(feature = "gui")]
use rodio::Source;
use std::sync::{Arc, Mutex};

/// Sample rate for audio output (Native Genesis FM rate: 53693175 / 7 / 144 = ~53267)
pub const SAMPLE_RATE: u32 = 53267;

/// Genesis Master Clock (NTSC)
pub const NTSC_MCLK: u32 = 53693175;
/// Genesis Master Clock (PAL)
pub const PAL_MCLK: u32 = 53203424;

/// Target frames per second
pub const FPS: u32 = 60;

/// Audio buffer size (in stereo sample pairs)
pub const BUFFER_SIZE: usize = 512;

/// Source for rodio that pulls from the emulator's ring buffer
#[cfg(feature = "gui")]
struct EmulatorSource {
    buffer: SharedAudioBuffer,
    sample_rate: u32,
    staging: Vec<f32>,
    staging_index: usize,
}

#[cfg(feature = "gui")]
impl Iterator for EmulatorSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.staging_index >= self.staging.len() {
            self.staging.resize(BUFFER_SIZE * 2, 0.0);
            if let Ok(mut buf) = self.buffer.lock() {
                buf.pop_f32(&mut self.staging);
            } else {
                self.staging.fill(0.0);
            }
            self.staging_index = 0;
        }

        let sample = self.staging[self.staging_index];
        self.staging_index += 1;
        Some(sample)
    }
}

#[cfg(feature = "gui")]
impl Source for EmulatorSource {
    fn current_frame_len(&self) -> Option<usize> {
        None // Unknown length
    }

    fn channels(&self) -> u16 {
        2 // Stereo
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Ring buffer for transferring audio samples between threads
#[derive(Debug)]
pub struct AudioBuffer {
    /// Sample storage (stereo i16)
    buffer: Vec<i16>,
    /// Write position
    write_pos: usize,
    /// Read position
    read_pos: usize,
    /// Number of samples available
    available: usize,
}

impl AudioBuffer {
    /// Create a new audio buffer
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0; capacity * 2], // Stereo
            write_pos: 0,
            read_pos: 0,
            available: 0,
        }
    }

    /// Push samples into the buffer
    pub fn push(&mut self, samples: &[i16]) {
        if self.buffer.is_empty() || samples.is_empty() {
            return;
        }

        let capacity = self.buffer.len();
        let samples = if samples.len() > capacity {
            &samples[samples.len() - capacity..]
        } else {
            samples
        };

        let overflow = samples
            .len()
            .saturating_sub(capacity.saturating_sub(self.available));
        if overflow > 0 {
            self.read_pos = (self.read_pos + overflow) % capacity;
            self.available -= overflow;
        }

        let samples_to_write = samples.len();
        let first_chunk_len = std::cmp::min(samples_to_write, capacity - self.write_pos);
        self.buffer[self.write_pos..self.write_pos + first_chunk_len]
            .copy_from_slice(&samples[..first_chunk_len]);

        let second_chunk_len = samples_to_write - first_chunk_len;
        if second_chunk_len > 0 {
            self.buffer[..second_chunk_len].copy_from_slice(&samples[first_chunk_len..]);
        }

        self.write_pos = (self.write_pos + samples_to_write) % capacity;
        self.available += samples_to_write;
    }

    /// Pop samples from the buffer into destination
    pub fn pop(&mut self, dest: &mut [i16]) {
        let samples_to_read = std::cmp::min(dest.len(), self.available);

        if samples_to_read > 0 {
            let first_chunk_len = std::cmp::min(samples_to_read, self.buffer.len() - self.read_pos);
            dest[..first_chunk_len]
                .copy_from_slice(&self.buffer[self.read_pos..self.read_pos + first_chunk_len]);

            let second_chunk_len = samples_to_read - first_chunk_len;
            if second_chunk_len > 0 {
                dest[first_chunk_len..samples_to_read]
                    .copy_from_slice(&self.buffer[..second_chunk_len]);
            }

            self.read_pos = (self.read_pos + samples_to_read) % self.buffer.len();
            self.available -= samples_to_read;
        }

        if samples_to_read < dest.len() {
            // Underrun - output silence for the remainder
            dest[samples_to_read..].fill(0);
        }
    }

    /// Pop samples as f32 (for cpal)
    pub fn pop_f32(&mut self, dest: &mut [f32]) {
        if self.available == 0 {
            dest.fill(0.0);
            return;
        }

        for sample in dest.iter_mut() {
            if self.available > 0 {
                let i16_sample = self.buffer[self.read_pos];
                self.read_pos = (self.read_pos + 1) % self.buffer.len();
                self.available -= 1;
                // Convert i16 to f32 [-1.0, 1.0]
                *sample = i16_sample as f32 / 32768.0;
            } else {
                *sample = 0.0;
            }
        }
    }

    /// Get number of available samples
    pub fn available(&self) -> usize {
        self.available
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.write_pos = 0;
        self.read_pos = 0;
        self.available = 0;
    }
}

/// Shared audio buffer type
pub type SharedAudioBuffer = Arc<Mutex<AudioBuffer>>;

/// Create a new shared audio buffer
pub fn create_audio_buffer() -> SharedAudioBuffer {
    Arc::new(Mutex::new(AudioBuffer::new(BUFFER_SIZE * 64)))
}

/// Audio output stream wrapper
#[cfg(feature = "gui")]
pub struct AudioOutput {
    _stream: rodio::OutputStream,
    _handle: rodio::OutputStreamHandle,
    _sink: rodio::Sink,
    pub sample_rate: u32,
}

#[cfg(feature = "gui")]
impl AudioOutput {
    /// Create a new audio output using rodio
    pub fn new(buffer: SharedAudioBuffer) -> Result<Self, String> {
        let host = cpal::default_host();

        let mut stream_and_rate = None;

        if let Some(device) = host.default_output_device() {
            if let Ok(config) = device.default_output_config() {
                if let Ok((stream, handle)) =
                    rodio::OutputStream::try_from_device_config(&device, config.clone())
                {
                    stream_and_rate = Some((stream, handle, config.sample_rate().0));
                }
            }
        }

        if stream_and_rate.is_none() {
            if let Ok(devices) = host.output_devices() {
                for device in devices {
                    let Ok(config) = device.default_output_config() else {
                        continue;
                    };
                    let Ok((stream, handle)) =
                        rodio::OutputStream::try_from_device_config(&device, config.clone())
                    else {
                        continue;
                    };
                    stream_and_rate = Some((stream, handle, config.sample_rate().0));
                    break;
                }
            }
        }

        let (stream, handle, sample_rate) = stream_and_rate
            .ok_or_else(|| "Failed to open audio output on any available device".to_string())?;

        let sink = rodio::Sink::try_new(&handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;

        let source = EmulatorSource {
            buffer,
            sample_rate,
            staging: Vec::new(),
            staging_index: 0,
        };

        // Use rodio's automatic resampling and channel mixing
        sink.append(source);
        sink.play();

        Ok(Self {
            _stream: stream,
            _handle: handle,
            _sink: sink,
            sample_rate,
        })
    }
}

/// Calculate samples needed per frame
/// Genesis runs at ~60fps NTSC, so samples_per_frame = sample_rate / 60
pub fn samples_per_frame() -> usize {
    (SAMPLE_RATE as f32 / 60.0).ceil() as usize
}

pub fn samples_per_frame_for_rate(sample_rate: u32) -> usize {
    (sample_rate as f32 / 60.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_create_audio_buffer() {
        let shared_buf = create_audio_buffer();
        let buf = shared_buf.lock().unwrap();

        // capacity is BUFFER_SIZE * 64 (512 * 64 = 32768)
        // internal buffer length is capacity * 2 for stereo (32768 * 2 = 65536)
        assert_eq!(buf.buffer.len(), BUFFER_SIZE * 128);
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.read_pos, 0);
        assert_eq!(buf.write_pos, 0);
    }

    #[test]
    fn test_create_audio_buffer_lock_state() {
        let shared_buf = create_audio_buffer();

        // Check initial ref counts
        assert_eq!(Arc::strong_count(&shared_buf), 1);
        assert_eq!(Arc::weak_count(&shared_buf), 0);

        // Verify successful lock acquisition
        let buf_lock = shared_buf.lock();
        assert!(
            buf_lock.is_ok(),
            "Mutex should not be poisoned and should be lockable"
        );

        let buf = buf_lock.unwrap();
        // Verify dimensions and capacity matches BUFFER_SIZE * 64 stereo (128)
        assert_eq!(buf.buffer.len(), BUFFER_SIZE * 128);
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.read_pos, 0);
        assert_eq!(buf.write_pos, 0);

        // Drop the lock so other threads could acquire it
        drop(buf);

        // Verify it can be shared and locked in a different thread
        let buf_clone = Arc::clone(&shared_buf);
        assert_eq!(Arc::strong_count(&shared_buf), 2);

        let handle = std::thread::spawn(move || {
            let buf_lock = buf_clone.lock();
            assert!(
                buf_lock.is_ok(),
                "Mutex should be lockable from another thread"
            );
        });

        handle.join().expect("Thread should not panic");
    }

    #[test]
    fn test_audio_buffer_new() {
        let buf = AudioBuffer::new(1024);
        assert_eq!(buf.available(), 0);
    }

    #[test]
    fn test_create_audio_buffer_layout() {
        let shared_buf = create_audio_buffer();
        let buf = shared_buf.lock().unwrap();

        // BUFFER_SIZE is 512, capacity passed to AudioBuffer::new is BUFFER_SIZE * 64.
        // Inside AudioBuffer::new, buffer length is capacity * 2.
        assert_eq!(buf.buffer.len(), BUFFER_SIZE * 64 * 2);
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.write_pos, 0);
        assert_eq!(buf.read_pos, 0);
    }

    #[test]
    fn test_audio_buffer_push_pop() {
        let mut buf = AudioBuffer::new(64);

        let samples = [100i16, 200, 300, 400];
        buf.push(&samples);

        assert_eq!(buf.available(), 4);

        let mut out = [0i16; 4];
        buf.pop(&mut out);

        assert_eq!(out, samples);
        assert_eq!(buf.available(), 0);
    }

    #[test]
    fn test_audio_buffer_underrun() {
        let mut buf = AudioBuffer::new(64);

        // Push only 2 samples
        buf.push(&[100i16, 200]);

        // Try to pop 4
        let mut out = [0i16; 4];
        buf.pop(&mut out);

        // First two should be valid, rest should be 0 (silence)
        assert_eq!(out[0], 100);
        assert_eq!(out[1], 200);
        assert_eq!(out[2], 0);
        assert_eq!(out[3], 0);
    }

    #[test]
    fn test_audio_buffer_wrap() {
        let mut buf = AudioBuffer::new(4); // 8 samples total (stereo)

        // Fill most of it
        buf.push(&[1i16, 2, 3, 4, 5, 6]);

        // Pop some
        let mut out = [0i16; 4];
        buf.pop(&mut out);

        // Push more (should wrap)
        buf.push(&[7i16, 8, 9, 10]);

        // Pop all
        let mut out2 = [0i16; 6];
        buf.pop(&mut out2);

        assert_eq!(out2[0], 5);
        assert_eq!(out2[1], 6);
        assert_eq!(out2[2], 7);
        assert_eq!(out2[3], 8);
        assert_eq!(out2[4], 9);
        assert_eq!(out2[5], 10);
    }

    #[test]
    fn test_samples_per_frame() {
        let spf = samples_per_frame();
        assert_eq!(spf, 888); // 53267 / 60 = 887.78 -> 888
    }

    #[test]
    fn test_pop_f32() {
        let mut buf = AudioBuffer::new(64);
        buf.push(&[16384i16, -16384]); // Half max positive/negative

        let mut out = [0.0f32; 2];
        buf.pop_f32(&mut out);

        assert!((out[0] - 0.5).abs() < 0.001);
        assert!((out[1] + 0.5).abs() < 0.001);
    }

    #[test]
    fn test_audio_buffer_clear() {
        let mut buf = AudioBuffer::new(64);

        // 1. Push data
        buf.push(&[10i16, 20]);

        // 2. Pop 1 sample (advances read_pos to 1)
        let mut out = [0i16; 1];
        buf.pop(&mut out);
        assert_eq!(out[0], 10);
        assert_eq!(buf.available(), 1);

        // 3. Clear the buffer
        buf.clear();

        // 4. Verify state reset
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.read_pos, 0);
        assert_eq!(buf.write_pos, 0);

        // 5. Verify pop returns silence
        let mut out_silence = [0i16; 1];
        buf.pop(&mut out_silence);
        assert_eq!(out_silence[0], 0);

        // 6. Verify reset state by pushing new data and checking order
        buf.push(&[30i16]);

        let mut out2 = [0i16; 1];
        buf.pop(&mut out2);

        assert_eq!(out2[0], 30);
        assert_eq!(buf.buffer[0], 30);
    }

    #[test]
    fn test_audio_buffer_clear_edge_cases() {
        let mut buf = AudioBuffer::new(2); // total capacity is 4 (stereo)

        // Edge Case 1: Completely full buffer
        buf.push(&[1i16, 2, 3, 4]);
        assert_eq!(buf.available(), 4);
        assert_eq!(buf.write_pos, 0); // wrapped around
        assert_eq!(buf.read_pos, 0);

        buf.clear();
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.write_pos, 0);
        assert_eq!(buf.read_pos, 0);

        // Edge Case 2: Wrap-around state (read_pos > write_pos)
        // Push 3 samples
        buf.push(&[10i16, 20, 30]);
        assert_eq!(buf.available(), 3);
        assert_eq!(buf.write_pos, 3);

        // Pop 2 samples (read_pos moves to 2)
        let mut out = [0i16; 2];
        buf.pop(&mut out);
        assert_eq!(buf.available(), 1);
        assert_eq!(buf.read_pos, 2);

        // Push 2 more samples (write_pos wraps to 1)
        buf.push(&[40i16, 50]);
        assert_eq!(buf.available(), 3);
        assert_eq!(buf.write_pos, 1);
        assert_eq!(buf.read_pos, 2);

        // Clear in wrap-around state
        buf.clear();
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.write_pos, 0);
        assert_eq!(buf.read_pos, 0);

        // Verify buffer works normally after wrap-around clear
        buf.push(&[100i16]);
        let mut out2 = [0i16; 1];
        buf.pop(&mut out2);
        assert_eq!(out2[0], 100);
    }

    #[test]
    fn test_audio_buffer_clear_completely_empty() {
        let mut buf = AudioBuffer::new(64);

        // Clear a brand new, empty buffer
        buf.clear();
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.read_pos, 0);
        assert_eq!(buf.write_pos, 0);
    }

    #[test]
    fn test_audio_buffer_clear_empty_shifted() {
        let mut buf = AudioBuffer::new(64);

        // Push 4, pop 4 so it's empty but read_pos and write_pos are 4
        buf.push(&[1i16, 2, 3, 4]);
        let mut out = [0i16; 4];
        buf.pop(&mut out);

        assert_eq!(buf.available(), 0);
        assert_eq!(buf.read_pos, 4);
        assert_eq!(buf.write_pos, 4);

        // Clear it
        buf.clear();
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.read_pos, 0);
        assert_eq!(buf.write_pos, 0);
    }

    #[test]
    fn test_audio_buffer_clear_partially_full() {
        let mut buf = AudioBuffer::new(64);

        // Push 4 samples, do not pop any
        buf.push(&[1i16, 2, 3, 4]);

        assert_eq!(buf.available(), 4);
        assert_eq!(buf.read_pos, 0);
        assert_eq!(buf.write_pos, 4);

        // Clear it
        buf.clear();
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.read_pos, 0);
        assert_eq!(buf.write_pos, 0);
    }

    #[test]
    fn test_pop_f32_precision_and_range() {
        let mut buf = AudioBuffer::new(64);

        // Test edge cases explicitly
        let samples = [i16::MAX, i16::MIN, 0, 1, -1];
        buf.push(&samples);

        let mut out = [0.0f32; 5];
        buf.pop_f32(&mut out);

        // i16::MAX is 32767. 32767 / 32768.0 = 0.999969482421875
        let expected_max = 32767.0 / 32768.0;
        assert!((out[0] - expected_max).abs() < f32::EPSILON);

        // i16::MIN is -32768. -32768 / 32768.0 = -1.0
        assert!((out[1] - (-1.0)).abs() < f32::EPSILON);

        // 0 / 32768.0 = 0.0
        assert!((out[2] - 0.0).abs() < f32::EPSILON);

        // 1 / 32768.0
        assert!((out[3] - (1.0 / 32768.0)).abs() < f32::EPSILON);

        // -1 / 32768.0
        assert!((out[4] - (-1.0 / 32768.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pop_f32_underrun() {
        let mut buf = AudioBuffer::new(64);
        buf.push(&[1000]);

        let mut out = [0.0f32; 3];
        buf.pop_f32(&mut out);

        // First sample should be valid
        assert!((out[0] - (1000.0 / 32768.0)).abs() < f32::EPSILON);
        // Rest should be silence (0.0)
        assert_eq!(out[1], 0.0);
        assert_eq!(out[2], 0.0);
    }

    #[test]
    fn test_pop_f32_exhaustive() {
        let mut buf = AudioBuffer::new(10);

        // Test all possible i16 values to ensure no precision issues across the entire range
        for sample in i16::MIN..=i16::MAX {
            buf.push(&[sample]);

            let mut out = [0.0f32; 1];
            buf.pop_f32(&mut out);

            let expected = sample as f32 / 32768.0;
            assert!(
                (out[0] - expected).abs() < f32::EPSILON,
                "Failed precision check for sample: {}",
                sample
            );

            // Verify range
            assert!(
                out[0] >= -1.0,
                "Failed lower bound check for sample: {}",
                sample
            );
            assert!(
                out[0] < 1.0,
                "Failed upper bound check for sample: {}",
                sample
            );

            // Ensure buffer state is clean
            assert_eq!(buf.available(), 0);
        }
    }

    #[test]
    fn test_push_overflow_keeps_newest_samples() {
        let mut buf = AudioBuffer::new(2);
        buf.push(&[1, 2, 3, 4, 5, 6]);

        let mut out = [0i16; 4];
        buf.pop(&mut out);

        assert_eq!(out, [3, 4, 5, 6]);
    }

    #[test]
    fn test_push_overflow_discards_oldest_buffered_samples() {
        let mut buf = AudioBuffer::new(2);
        buf.push(&[10, 20, 30]);
        buf.push(&[40, 50, 60]);

        let mut out = [0i16; 4];
        buf.pop(&mut out);

        assert_eq!(out, [30, 40, 50, 60]);
    }

    #[test]
    fn test_pop_f32_boundaries() {
        let mut buf = AudioBuffer::new(4);

        // Push extreme values: MIN (-32768), MAX (32767), 0, and a mid-value
        let samples = [i16::MIN, i16::MAX, 0, 16384];
        buf.push(&samples);

        let mut dest = [0.0f32; 4];
        buf.pop_f32(&mut dest);

        // MIN should be exactly -1.0
        assert!((dest[0] - (-1.0)).abs() < f32::EPSILON);
        // MAX should be 32767 / 32768
        assert!((dest[1] - (32767.0 / 32768.0)).abs() < f32::EPSILON);
        // 0 should be exactly 0.0
        assert!((dest[2] - 0.0).abs() < f32::EPSILON);
        // 16384 should be exactly 0.5
        assert!((dest[3] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pop_f32_wrap_around() {
        let mut buf = AudioBuffer::new(2); // capacity 4 samples (2*2)

        // Push 3 samples
        buf.push(&[16384, 16384, 16384]);

        // Pop 2
        let mut dest2 = [0.0f32; 2];
        buf.pop_f32(&mut dest2);
        assert!((dest2[0] - 0.5).abs() < f32::EPSILON);
        assert!((dest2[1] - 0.5).abs() < f32::EPSILON);

        // Push 2 more (wraps)
        buf.push(&[-16384, -32768]);

        // Pop 3 (should get the one remaining + two new ones)
        let mut dest3 = [0.0f32; 3];
        buf.pop_f32(&mut dest3);

        assert!((dest3[0] - 0.5).abs() < f32::EPSILON);
        assert!((dest3[1] - (-0.5)).abs() < f32::EPSILON);
        assert!((dest3[2] - (-1.0)).abs() < f32::EPSILON);
    }

    // Reference implementation (old slow loop) for property testing
    fn push_reference(buffer: &mut AudioBuffer, samples: &[i16]) {
        if buffer.buffer.is_empty() || samples.is_empty() {
            return;
        }

        let capacity = buffer.buffer.len();
        let samples = if samples.len() > capacity {
            &samples[samples.len() - capacity..]
        } else {
            samples
        };

        let overflow = samples
            .len()
            .saturating_sub(capacity.saturating_sub(buffer.available));
        if overflow > 0 {
            buffer.read_pos = (buffer.read_pos + overflow) % capacity;
            buffer.available -= overflow;
        }

        for &sample in samples {
            buffer.buffer[buffer.write_pos] = sample;
            buffer.write_pos = (buffer.write_pos + 1) % capacity;
            buffer.available += 1;
        }
    }

    proptest! {
        #[test]
        fn test_push_equivalence(
            buffer_size in 10usize..1000usize,
            ref initial_fill in prop::collection::vec(any::<i16>(), 0..1000),
            ref push_data in prop::collection::vec(any::<i16>(), 0..2000)
        ) {
            // Setup two identical buffers
            let mut buf1 = AudioBuffer::new(buffer_size);
            let mut buf2 = AudioBuffer::new(buffer_size);

            // Pre-fill both buffers identically
            push_reference(&mut buf1, initial_fill);
            push_reference(&mut buf2, initial_fill);

            // Verify they start identical
            prop_assert_eq!(&buf1.buffer, &buf2.buffer);
            prop_assert_eq!(buf1.write_pos, buf2.write_pos);
            prop_assert_eq!(buf1.available, buf2.available);

            // Apply push operation
            // buf1 uses reference implementation
            push_reference(&mut buf1, push_data);
            // buf2 uses new optimized implementation
            buf2.push(push_data);

            // Verify they end up identical
            prop_assert_eq!(&buf1.buffer, &buf2.buffer, "Buffer content mismatch");
            prop_assert_eq!(buf1.write_pos, buf2.write_pos, "Write position mismatch");
            prop_assert_eq!(buf1.available, buf2.available, "Available count mismatch");
        }
    }
}
