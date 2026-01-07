//! Audio Processing Unit
//!
//! The Genesis APU consists of:
//! - Yamaha YM2612: 6-channel FM synthesis
//! - Texas Instruments SN76489: 4-channel PSG (3 tone + 1 noise)

pub mod ym2612;
pub mod psg;

pub use ym2612::Ym2612;
pub use psg::Psg;

/// Combined APU state
#[derive(Debug, Default)]
pub struct Apu {
    /// YM2612 FM chip
    pub ym2612: Ym2612,
    /// SN76489 PSG chip
    pub psg: Psg,
}

impl Apu {
    /// Create a new APU
    pub fn new() -> Self {
        Self {
            ym2612: Ym2612::new(),
            psg: Psg::new(),
        }
    }
    
    /// Reset both sound chips
    pub fn reset(&mut self) {
        self.ym2612.reset();
        self.psg.reset();
    }
    
    /// Generate audio samples
    /// 
    /// # Arguments
    /// * `buffer` - Output buffer for stereo samples (L, R, L, R, ...)
    /// * `sample_count` - Number of stereo sample pairs to generate
    pub fn generate_samples(&mut self, buffer: &mut [i16], sample_count: usize) {
        for i in 0..sample_count {
            let fm_sample = self.ym2612.step();
            let psg_sample = self.psg.step();
            
            // Mix FM and PSG
            let mixed = ((fm_sample as i32) + (psg_sample as i32)).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            
            // Stereo output (duplicate for now, proper panning later)
            let idx = i * 2;
            if idx + 1 < buffer.len() {
                buffer[idx] = mixed;
                buffer[idx + 1] = mixed;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_apu_new() {
        let apu = Apu::new();
        assert!(!apu.ym2612.dac_enable);
    }
    
    #[test]
    fn test_apu_reset() {
        let mut apu = Apu::new();
        apu.ym2612.dac_enable = true;
        apu.reset();
        assert!(!apu.ym2612.dac_enable);
    }
    
    #[test]
    fn test_apu_generate_samples() {
        let mut apu = Apu::new();
        let mut buffer = [0i16; 64];
        apu.generate_samples(&mut buffer, 32);
        // Just verify it doesn't crash
    }
}
