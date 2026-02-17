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
use psg::Psg;
use serde_json::{json, Value};
use ym2612::{Bank, Ym2612};

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

    pub fn write_fm_addr(&mut self, bank: Bank, data: u8) {
        self.fm.write_addr(bank, data);
    }

    pub fn write_fm_data(&mut self, bank: Bank, data: u8) {
        self.fm.write_data_bank(bank, data);
    }


    /// Run one sample cycle (at ~44100Hz or system clock)
    /// Returns a stereo mixed sample pair.
    pub fn step(&mut self) -> (i16, i16) {
        // Generate FM sample
        let (fm_l, fm_r) = self.fm.generate_sample();

        // Step the components
        self.fm.step(1); // 1 "cycle" per sample step (simplified)

        // Generate PSG sample (mono)
        let psg_sample = self.psg.step();

        // Mix: convert to i32 to prevent early overflow, then clamp
        let left = (fm_l as i32 + psg_sample as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let right =
            (fm_r as i32 + psg_sample as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;

        (left, right)
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
        apu.write_fm_addr(Bank::Bank0, 0x28);
        apu.write_fm_data(Bank::Bank0, 0xF0); // Key on Ch 1
        assert_eq!(apu.fm.registers[0][0x28], 0xF0);
    }
}
