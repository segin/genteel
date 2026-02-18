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
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ym2612::{Bank, Ym2612};

#[derive(Debug, Serialize, Deserialize)]
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
        serde_json::to_value(self).unwrap()
    }

    fn write_state(&mut self, state: &Value) {
        if let Ok(new_apu) = serde_json::from_value(state.clone()) {
            *self = new_apu;
        }
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

    #[test]
    fn test_debuggable_roundtrip() {
        let mut apu = Apu::new();

        // Modify PSG state
        // Set Channel 0 Frequency to 0x3F5 (1013)
        // 1. Latch Channel 0 (00), Freq (0), Data (0101) -> 1000 0101 -> 0x85
        apu.write_psg(0x85);
        // 2. Data Byte: High 6 bits (0011 11) -> 0011 1111 -> 0x3F
        apu.write_psg(0x3F);

        // Set Channel 1 Volume to 8
        // Channel 1 (01), Volume (1), Data (1000) -> 1011 1000 -> 0xB8
        apu.write_psg(0xB8);

        // Modify FM state
        // Bank 0, Register 0x22 (LFO) = 0x08 (Enabled)
        apu.write_fm_addr(Bank::Bank0, 0x22);
        apu.write_fm_data(Bank::Bank0, 0x08);

        // Serialize state
        let state = apu.read_state();

        // Deserialize into new APU
        let mut restored_apu = Apu::new();
        restored_apu.write_state(&state);

        // Verify PSG state restoration
        assert_eq!(restored_apu.psg.tones[0].frequency, 0x3F5);
        assert_eq!(restored_apu.psg.tones[1].volume, 8);

        // Verify FM state restoration
        assert_eq!(restored_apu.fm.registers[0][0x22], 0x08);
    }
}
