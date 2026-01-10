//! Audio Output Module
//!
//! Provides SDL2 audio callback integration for Genesis audio output.
//! Uses a ring buffer to transfer samples from emulation thread to audio callback.

use std::sync::{Arc, Mutex};

/// Sample rate for audio output (Genesis runs at ~53kHz internally, we resample)
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

/// Audio callback function for SDL2
/// This is called from the audio thread to fill the output buffer
pub fn audio_callback(buffer: &SharedAudioBuffer, out: &mut [i16]) {
    if let Ok(mut buf) = buffer.lock() {
        buf.pop(out);
    } else {
        // Lock failed, output silence
        for sample in out.iter_mut() {
            *sample = 0;
        }
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
}
