//! Audio Output Module
//!
//! Provides cross-platform audio output using cpal.
//! Uses a ring buffer to transfer samples from emulation thread to audio callback.

#[cfg(feature = "gui")]
use rodio::Source;
use std::sync::{Arc, Mutex};

/// Sample rate for audio output
pub const SAMPLE_RATE: u32 = 44100;

/// Audio buffer size (in stereo sample pairs)
pub const BUFFER_SIZE: usize = 2048;

/// Source for rodio that pulls from the emulator's ring buffer
#[cfg(feature = "gui")]
struct EmulatorSource {
    buffer: SharedAudioBuffer,
}

#[cfg(feature = "gui")]
impl Iterator for EmulatorSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = self.buffer.lock().ok()?;
        if buf.available > 0 {
            let i16_sample = buf.buffer[buf.read_pos];
            buf.read_pos = (buf.read_pos + 1) % buf.buffer.len();
            buf.available -= 1;
            Some(i16_sample as f32 / 32768.0)
        } else {
            // Underflow - return silence instead of None to keep the stream alive
            Some(0.0)
        }
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
        SAMPLE_RATE
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
    _stream: rodio::OutputStream,
    _handle: rodio::OutputStreamHandle,
    _sink: rodio::Sink,
    pub sample_rate: u32,
}

#[cfg(feature = "gui")]
impl AudioOutput {
    /// Create a new audio output using rodio
    pub fn new(buffer: SharedAudioBuffer) -> Result<Self, String> {
        let (stream, handle) = rodio::OutputStream::try_default()
            .map_err(|e| format!("Failed to open audio output: {}", e))?;

        let sink = rodio::Sink::try_new(&handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;

        let source = EmulatorSource { buffer };

        // Use rodio's automatic resampling and channel mixing
        sink.append(source);
        sink.play();

        Ok(Self {
            _stream: stream,
            _handle: handle,
            _sink: sink,
            sample_rate: SAMPLE_RATE,
        })
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
}
