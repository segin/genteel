//! Audio Output Module
//!
//! Provides cross-platform audio output using cpal.
//! Uses a ring buffer to transfer samples from emulation thread to audio callback.

#[cfg(feature = "gui")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

/// Sample rate for audio output
pub const SAMPLE_RATE: u32 = 44100;

/// Audio buffer size (in stereo sample pairs)
pub const BUFFER_SIZE: usize = 2048;

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
        for &sample in samples {
            if self.available < self.buffer.len() {
                self.buffer[self.write_pos] = sample;
                self.write_pos = (self.write_pos + 1) % self.buffer.len();
                self.available += 1;
            }
        }
    }

    /// Pop samples from the buffer into destination
    pub fn pop(&mut self, dest: &mut [i16]) {
        for sample in dest.iter_mut() {
            if self.available > 0 {
                *sample = self.buffer[self.read_pos];
                self.read_pos = (self.read_pos + 1) % self.buffer.len();
                self.available -= 1;
            } else {
                // Underrun - output silence
                *sample = 0;
            }
        }
    }

    /// Pop samples as f32 (for cpal)
    pub fn pop_f32(&mut self, dest: &mut [f32]) {
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
    Arc::new(Mutex::new(AudioBuffer::new(BUFFER_SIZE * 4)))
}

/// Audio output stream wrapper
#[cfg(feature = "gui")]
pub struct AudioOutput {
    _stream: cpal::Stream,
}

#[cfg(feature = "gui")]
impl AudioOutput {
    /// Create a new audio output using cpal
    pub fn new(buffer: SharedAudioBuffer) -> Result<Self, String> {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .ok_or("No audio output device found")?;

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        let buffer_clone = buffer.clone();

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if let Ok(mut buf) = buffer_clone.lock() {
                        buf.pop_f32(data);
                    } else {
                        // Lock failed, output silence
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                    }
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;

        Ok(Self { _stream: stream })
    }
}

/// Calculate samples needed per frame
/// Genesis runs at ~60fps NTSC, so samples_per_frame = sample_rate / 60
pub fn samples_per_frame() -> usize {
    (SAMPLE_RATE / 60) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_new() {
        let buf = AudioBuffer::new(1024);
        assert_eq!(buf.available(), 0);
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
        assert_eq!(spf, 735); // 44100 / 60 = 735
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

        // Push some data
        let samples = [100i16, 200, 300, 400];
        buf.push(&samples);

        assert_eq!(buf.available(), 4);
        assert_ne!(buf.write_pos, 0);

        // Clear the buffer
        buf.clear();

        // Verify state reset
        assert_eq!(buf.available(), 0);
        assert_eq!(buf.read_pos, 0);
        assert_eq!(buf.write_pos, 0);

        // Verify pop returns silence
        let mut out = [0i16; 4];
        buf.pop(&mut out);
        assert_eq!(out, [0, 0, 0, 0]);

        // Verify next push starts at 0
        buf.push(&[500, 600]);
        assert_eq!(buf.write_pos, 2);
        assert_eq!(buf.buffer[0], 500);
        assert_eq!(buf.buffer[1], 600);
    }
}
