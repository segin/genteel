//! Audio Processing Unit (APU)
//!
//! The APU orchestrates the sound components:
//! - Z80 CPU (managed separately in `src/z80`, but interfaced here if needed)
//! - YM2612 FM Synthesizer
//! - SN76489 PSG
//!
//! It handles routing of register writes and audio sample generation.

pub mod sn76489;
pub mod ym2612;

use sn76489::Sn76489;
use ym2612::Ym2612;

#[derive(Debug)]
pub struct Apu {
    pub psg: Sn76489,
    pub fm: Ym2612,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            psg: Sn76489::new(),
            fm: Ym2612::new(),
        }
    }

    pub fn reset(&mut self) {
        self.psg.reset();
        self.fm.reset();
    }

    // === PSG Interface ===
    pub fn write_psg(&mut self, data: u8) {
        self.psg.write(data);
    }

    // === YM2612 Interface ===
    pub fn read_fm_status(&self) -> u8 {
        self.fm.read_status()
    }

    pub fn write_fm_addr0(&mut self, data: u8) {
        self.fm.write_addr0(data);
    }

    pub fn write_fm_data0(&mut self, data: u8) {
        self.fm.write_data0(data);
    }

    pub fn write_fm_addr1(&mut self, data: u8) {
        self.fm.write_addr1(data);
    }

    pub fn write_fm_data1(&mut self, data: u8) {
        self.fm.write_data1(data);
    }

    /// Run one sample cycle (at ~44100Hz or system clock)
    /// Returns a mixed sample.
    pub fn step(&mut self) -> i16 {
        // Placeholder: Return silence
        // Real impl would require stepping PSG and FM generators
        0
    }
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization() {
        let apu = Apu::new();
        // PSG should be silent
        assert_eq!(apu.psg.tone1_vol, 0x0F);
        // FM status should be clean
        assert_eq!(apu.fm.status, 0);
    }

    #[test]
    fn test_psg_passthrough() {
        let mut apu = Apu::new();
        apu.write_psg(0x8F); // Latch Tone 1 Vol to 15 (Silent)
        apu.write_psg(0x90); // Latch Tone 1 Vol to 0 (Loud)
        assert_eq!(apu.psg.tone1_vol, 0);
    }

    #[test]
    fn test_fm_passthrough() {
        let mut apu = Apu::new();
        apu.write_fm_addr0(0x28);
        apu.write_fm_data0(0xF0); // Key on Ch 1
        assert_eq!(apu.fm.registers[0][0x28], 0xF0);
    }
}
