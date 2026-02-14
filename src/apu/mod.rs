//! Audio Processing Unit (APU)
//!
//! The APU orchestrates the sound components:
//! - Z80 CPU (managed separately in `src/z80`, but interfaced here if needed)
//! - YM2612 FM Synthesizer
//! - SN76489 PSG
//!
//! It handles routing of register writes and audio sample generation.

pub mod psg;
pub mod ym2612;

use crate::debugger::Debuggable;
use serde_json::{json, Value};
use psg::Psg;
use ym2612::Ym2612;

#[derive(Debug)]
pub struct Apu {
    pub psg: Psg,
    pub fm: Ym2612,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            psg: Psg::new(),
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
        // Step the components
        self.fm.step(1); // 1 "cycle" per sample step (simplified)

        self.psg.step()
    }
}

impl Debuggable for Apu {
    fn read_state(&self) -> Value {
        json!({
            "psg": {
                "tone1_freq": self.psg.tones[0].frequency,
                "tone1_vol": self.psg.tones[0].volume,
                "tone2_freq": self.psg.tones[1].frequency,
                "tone3_freq": self.psg.tones[2].frequency,
                "noise_white": self.psg.noise.white_noise,
                "noise_rate": self.psg.noise.shift_rate,
            },
            "fm": {
                "status": self.fm.status,
                // Dumping all registers is too heavy, maybe just status
            }
        })
    }

    fn write_state(&mut self, _state: &Value) {
        // Read-only for now
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
        assert_eq!(apu.psg.tones[0].volume, 0x0F);
        // FM status should be clean
        assert_eq!(apu.fm.status, 0);
    }

    #[test]
    fn test_psg_passthrough() {
        let mut apu = Apu::new();
        apu.write_psg(0x8F); // Latch Tone 1 Vol to 15 (Silent)
        apu.write_psg(0x90); // Latch Tone 1 Vol to 0 (Loud)
        assert_eq!(apu.psg.tones[0].volume, 0);
    }

    #[test]
    fn test_fm_passthrough() {
        let mut apu = Apu::new();
        apu.write_fm_addr0(0x28);
        apu.write_fm_data0(0xF0); // Key on Ch 1
        assert_eq!(apu.fm.registers[0][0x28], 0xF0);
    }
}
pub mod psg;
