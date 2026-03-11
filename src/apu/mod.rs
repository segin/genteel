pub mod blip_buf;
pub mod psg;
pub mod ym2612;

use crate::apu::psg::Psg;
use crate::apu::ym2612::Ym2612;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::debugger::Debuggable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bank {
    Bank0 = 0,
    Bank1 = 1,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Apu {
    pub fm: Ym2612,
    pub psg: Psg,
    /// Visualize channel outputs [0-5: FM, 6-9: PSG]
    pub channel_buffers: Vec<Vec<i16>>,
    pub buffer_idx: usize,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            fm: Ym2612::new(),
            psg: Psg::new(),
            channel_buffers: vec![vec![0; 128]; 10],
            buffer_idx: 0,
        }
    }

    pub fn reset(&mut self) {
        self.fm.reset();
        self.psg.reset();
    }

    pub fn write_psg(&mut self, data: u8) {
        self.psg.write(data);
    }

    pub fn read_fm_status(&self) -> u8 {
        self.fm.read_status()
    }

    pub fn write_fm_addr(&mut self, bank: Bank, data: u8) {
        self.fm.write_addr(bank, data);
    }

    pub fn write_fm_data(&mut self, bank: Bank, data: u8) {
        self.fm.write_data_bank(bank, data);
    }

    pub fn tick_cycles(&mut self, m68k_cycles: u32) {
        self.fm.step(m68k_cycles);
        self.psg.step_cycles(m68k_cycles);
    }

    /// Attempts to generate a mixed audio sample pair.
    pub fn generate_sample(&mut self) -> Option<(i16, i16)> {
        // Try to read from FM blip buffers
        let mut fm_l = [0i16; 1];
        let mut fm_r = [0i16; 1];
        
        if self.fm.blip_l.read_samples(&mut fm_l) > 0 {
            self.fm.blip_r.read_samples(&mut fm_r);
            self.fm.total_clocks = 0;

            // PSG should ideally be synced to FM sample rate
            let psg_val = self.psg.current_sample();

            // Update visualization every 128 samples approx
            let fm_samples = self.fm.generate_channel_samples();
            let psg_samples = self.psg.get_channel_samples();
            for i in 0..6 { self.channel_buffers[i][self.buffer_idx] = fm_samples[i]; }
            for i in 0..4 { self.channel_buffers[6 + i][self.buffer_idx] = psg_samples[i]; }
            self.buffer_idx = (self.buffer_idx + 1) % 128;

            let left = (fm_l[0] as i32 + psg_val as i32).clamp(-32768, 32767) as i16;
            let right = (fm_r[0] as i32 + psg_val as i32).clamp(-32768, 32767) as i16;

            Some((left, right))
        } else {
            None
        }
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
        assert_eq!(apu.psg.tones[0].volume, 0x0F);
        assert_eq!(apu.fm.status, 0);
    }

    #[test]
    fn test_psg_passthrough() {
        let mut apu = Apu::new();
        apu.write_psg(0x8F);
        apu.write_psg(0x90);
        assert_eq!(apu.psg.tones[0].volume, 0);
    }

    #[test]
    fn test_fm_passthrough() {
        let mut apu = Apu::new();
        apu.write_fm_addr(Bank::Bank0, 0x28);
        apu.write_fm_data(Bank::Bank0, 0xF0);
        assert_eq!(apu.fm.registers[0][0x28], 0xF0);
    }
}
