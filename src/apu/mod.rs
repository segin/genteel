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

#[cfg(test)]
mod tests_psg_expansion;
#[cfg(test)]
mod tests_ym2612_expansion;

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
    pub fn step(&mut self, m68k_cycles: u32) -> (i16, i16) {
        // 1. Step the components
        self.fm.step(m68k_cycles);

        // PSG is clocked at 3.58MHz (M68k/2)
        let psg_cycles = m68k_cycles / 2;
        let psg_sample = self.psg.step_cycles(psg_cycles);

        // 2. Generate samples from new state
        let (fm_l, fm_r) = self.fm.generate_sample();

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
    fn test_debuggable_implementation() {
        let mut apu = Apu::new();

        // 1. Modify State
        // PSG: Channel 0 Volume = 0 (Max), Freq = 0x123
        apu.write_psg(0x90); // Vol 0
        apu.write_psg(0x83); // Freq Low 3
        apu.write_psg(0x12); // Freq High 0x12

        // FM: Write to Bank 0, Reg 0x30, Data 0x77
        apu.write_fm_addr(Bank::Bank0, 0x30);
        apu.write_fm_data(Bank::Bank0, 0x77);

        // 2. Read State
        let state = apu.read_state();

        // Verify JSON structure
        assert!(state.get("psg").is_some());
        assert!(state.get("fm").is_some());

        // 3. Write State to New Instance
        let mut new_apu = Apu::new();
        new_apu.write_state(&state);

        // 4. Verify Restoration
        // PSG
        assert_eq!(new_apu.psg.tones[0].volume, 0);
        assert_eq!(new_apu.psg.tones[0].frequency, 0x123);

        // FM
        assert_eq!(new_apu.fm.registers[0][0x30], 0x77);

        // Full State Equality Check (via JSON representation)
        assert_eq!(apu.read_state(), new_apu.read_state());
    }
}
