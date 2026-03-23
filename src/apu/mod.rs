//! Audio Processing Unit (APU)
//!
//! Refactored to use band-limited synthesis for both FM and PSG.

pub mod blip_buf;
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
    #[serde(skip, default = "default_channel_buffers")]
    pub channel_buffers: [[i16; 128]; 10],
    #[serde(skip)]
    pub buffer_idx: usize,
}

fn default_channel_buffers() -> [[i16; 128]; 10] {
    [[0i16; 128]; 10]
}

impl Apu {
    pub fn new() -> Self {
        Self {
            psg: Psg::new(),
            fm: Ym2612::new(),
            channel_buffers: [[0; 128]; 10],
            buffer_idx: 0,
        }
    }

    pub fn reset(&mut self) {
        self.psg.reset();
        self.fm.reset();
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
        let psg_cycles = m68k_cycles / 2;
        self.psg.step_cycles(psg_cycles);
    }

    /// Attempts to generate a mixed audio sample pair.
    /// Returns `Some((left, right))` if a sample is available in the blip buffers,
    /// otherwise returns `None`.
    pub fn generate_sample(&mut self) -> Option<(i16, i16)> {
        let mut fm_l = [0i16; 1];
        let mut fm_r = [0i16; 1];
        let mut psg_buf = [0i16; 1];

        // We check FM left as the primary clock
        if self.fm.blip_l.read_samples(&mut fm_l) > 0 {
            self.fm.blip_r.read_samples(&mut fm_r);
            self.psg.blip.read_samples(&mut psg_buf);

            let fm_l_val = fm_l[0];
            let fm_r_val = fm_r[0];
            let psg_val = psg_buf[0];

            // Update visualization
            let fm_samples = self.fm.generate_channel_samples();
            let psg_samples = self.psg.get_channel_samples();
            for i in 0..6 {
                self.channel_buffers[i][self.buffer_idx] = fm_samples[i];
            }
            for i in 0..4 {
                self.channel_buffers[6 + i][self.buffer_idx] = psg_samples[i];
            }
            self.buffer_idx = (self.buffer_idx + 1) % 128;

            let left = (fm_l_val as i32 + psg_val as i32).clamp(-32768, 32767) as i16;
            let right = (fm_r_val as i32 + psg_val as i32).clamp(-32768, 32767) as i16;

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
        if let Ok(new_apu) = Apu::deserialize(state) {
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

    #[test]
    fn test_write_fm_addr() {
        let mut apu = Apu::new();

        // Write address to Bank0, and check if data written goes to this address
        apu.write_fm_addr(Bank::Bank0, 0x22);
        apu.write_fm_data(Bank::Bank0, 0x11);
        assert_eq!(apu.fm.registers[0][0x22], 0x11);

        // Change address in Bank0, write data, check new address is used
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.write_fm_data(Bank::Bank0, 0x33);
        assert_eq!(apu.fm.registers[0][0x27], 0x33);

        // Verify that Bank1 operates independently
        apu.write_fm_addr(Bank::Bank1, 0x28);
        apu.write_fm_data(Bank::Bank1, 0x44);
        assert_eq!(apu.fm.registers[1][0x28], 0x44);

        // Change address in Bank1
        apu.write_fm_addr(Bank::Bank1, 0x2B);
        apu.write_fm_data(Bank::Bank1, 0x55);
        assert_eq!(apu.fm.registers[1][0x2B], 0x55);

        // Ensure Bank0 address wasn't affected by Bank1 address writes
        apu.write_fm_data(Bank::Bank0, 0x66);
        assert_eq!(apu.fm.registers[0][0x27], 0x66); // The last address set for Bank0 was 0x27
    }

    #[test]
    fn test_write_fm_data() {
        let mut apu = Apu::new();

        // Write to Bank0, Register 0x24 (Timer A High)
        apu.write_fm_addr(Bank::Bank0, 0x24);
        apu.write_fm_data(Bank::Bank0, 0xAA);
        assert_eq!(apu.fm.registers[0][0x24], 0xAA);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag should be set

        apu.tick_cycles(32); // clear busy

        // Write to Bank1, Register 0x24
        apu.write_fm_addr(Bank::Bank1, 0x24);
        apu.write_fm_data(Bank::Bank1, 0xBB);
        assert_eq!(apu.fm.registers[1][0x24], 0xBB);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag should be set
    }

    #[test]
    fn test_apu_write_fm_data_delegation_side_effects() {
        let mut apu = Apu::new();

        // 1. Test DAC Enable (Bank0, Register 0x2B)
        apu.write_fm_addr(Bank::Bank0, 0x2B);
        apu.write_fm_data(Bank::Bank0, 0x80); // Enable DAC
        assert_eq!(apu.fm.registers[0][0x2B], 0x80);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag set
        apu.tick_cycles(32); // clear busy

        // 2. Test DAC Value (Bank0, Register 0x2A)
        apu.write_fm_addr(Bank::Bank0, 0x2A);
        apu.write_fm_data(Bank::Bank0, 0xFF); // Set DAC value to maximum
        assert_eq!(apu.fm.registers[0][0x2A], 0xFF);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag set
        apu.tick_cycles(32); // clear busy

        // 3. Test Panning Update (Bank1, Register 0xB6)
        apu.write_fm_addr(Bank::Bank1, 0xB6);
        apu.write_fm_data(Bank::Bank1, 0xC0); // Left and Right panning
        assert_eq!(apu.fm.registers[1][0xB6], 0xC0);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag set
    }

    #[test]
    fn test_read_fm_status() {
        let mut apu = Apu::new();
        // Initial status should be 0
        assert_eq!(apu.read_fm_status(), 0);

        // Writing FM data should set the busy bit (bit 7)
        apu.write_fm_data(Bank::Bank0, 0);
        assert!((apu.read_fm_status() & 0x80) != 0);

        // Tick cycles to clear busy bit (busy is 224, mclks is cycles * 7, 32 cycles should clear it)
        apu.tick_cycles(32);
        assert_eq!(apu.read_fm_status() & 0x80, 0);

        // Test Timer A
        // Timer A is set via 0x24 (bits 9-2) and 0x25 (bits 1-0)
        // Set it to a very small value to trigger quickly
        apu.write_fm_addr(Bank::Bank0, 0x24);
        apu.write_fm_data(Bank::Bank0, 0xFF);
        apu.write_fm_addr(Bank::Bank0, 0x25);
        apu.write_fm_data(Bank::Bank0, 0x03); // Max value is 1023 (0x3FF)

        // Enable and trigger timer A (bit 0 = enable, bit 2 = load bit)
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.write_fm_data(Bank::Bank0, 0x05);

        // After some cycles, bit 0 should be set
        // Period is (1024 - 1023) * 72 = 72 master clocks.
        // tick_cycles(20) should be enough (20 * 7 = 140 master clocks)
        apu.tick_cycles(20);
        assert!((apu.read_fm_status() & 0x01) != 0);

        // Test Timer B
        // Reset status (YM2612 reset status bits via register 0x27 bits 4 and 5)
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.write_fm_data(Bank::Bank0, 0x30);
        assert_eq!(apu.read_fm_status() & 0x03, 0);

        // Set Timer B to max value (255)
        apu.write_fm_addr(Bank::Bank0, 0x26);
        apu.write_fm_data(Bank::Bank0, 0xFF);

        // Enable and trigger Timer B (bit 1 = enable, bit 3 = load bit)
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.write_fm_data(Bank::Bank0, 0x0A);

        // Period is (256 - 255) * 1152 = 1152 master clocks.
        // tick_cycles(200) should be enough (200 * 7 = 1400 master clocks)
        apu.tick_cycles(200);
        assert!((apu.read_fm_status() & 0x02) != 0);
    }

    #[test]
    fn test_write_fm_data_side_effects() {
        let mut apu = Apu::new();

        // Enable DAC
        apu.write_fm_addr(Bank::Bank0, 0x2B);
        apu.write_fm_data(Bank::Bank0, 0x80);

        // Set DAC value to a non-zero amplitude
        apu.write_fm_addr(Bank::Bank0, 0x2A);
        apu.write_fm_data(Bank::Bank0, 0xFF);

        // Pan Left Only to test specific output
        apu.write_fm_addr(Bank::Bank1, 0xB6);
        apu.write_fm_data(Bank::Bank1, 0x80);

        // Tick cycles to allow YM2612 to generate samples
        apu.tick_cycles(1);

        // Assert DAC output is observable in the blip buffer
        assert!(apu.fm.blip_l.read_instant() > 0, "Left audio should be positive due to DAC");
        assert_eq!(apu.fm.blip_r.read_instant(), 0, "Right audio should be zero due to panning");
    }
}

#[cfg(test)]
mod tests_generate_sample {
    use super::*;
    use ym2612::Bank;

    #[test]
    fn test_generate_sample() {
        let mut apu = Apu::new();

        // At start, the buffer is full of 0s.
        // generate_sample should return Some((0,0)).
        let sample = apu.generate_sample();
        assert_eq!(sample, Some((0, 0)));

        // Setup FM DAC to generate sound (delta != 0)
        apu.write_fm_addr(Bank::Bank0, 0x2B);
        apu.write_fm_data(Bank::Bank0, 0x80); // Enable DAC
        apu.write_fm_addr(Bank::Bank0, 0x2A);
        apu.write_fm_data(Bank::Bank0, 0xFF); // Write a high DAC value

        // Setup PSG tone to generate sound
        apu.write_psg(0x8A); // Ch0 freq low
        apu.write_psg(0x00); // Ch0 freq high
        apu.write_psg(0x90); // Ch0 vol max

        let mut has_non_zero = false;
        // Tick enough cycles to produce changes and flush out the zeros
        for _ in 0..2000 {
            apu.tick_cycles(1);
            if let Some((l, r)) = apu.generate_sample() {
                if l != 0 || r != 0 {
                    has_non_zero = true;
                }
            }
        }

        assert!(has_non_zero, "Expected non-zero audio samples after setting up APU channels");
    }

    #[test]
    fn test_generate_sample_none() {
        let mut apu = Apu::new();
        let mut sample_count = 0;

        // Read samples until we exhaust the initial pre-allocated buffers
        while apu.generate_sample().is_some() {
            sample_count += 1;
            if sample_count > 10000 {
                panic!("Infinite loop reading samples");
            }
        }

        assert_eq!(apu.generate_sample(), None, "Should return None when buffers are empty");
    }

    #[test]
    fn test_generate_sample_clamping() {
        let mut apu = Apu::new();

        // Drain the initial buffer
        while apu.generate_sample().is_some() {}

        // Add large deltas to force overflow and clamping to the upper limit
        // Using `add_delta(0, ...)` as the immediate accumulation point before ticking
        apu.fm.blip_l.add_delta(0, 32767);
        apu.fm.blip_r.add_delta(0, -32768);
        apu.psg.blip.add_delta(0, 32767);

        apu.tick_cycles(100);

        let mut sample = None;
        for _ in 0..100 {
            if let Some(s) = apu.generate_sample() {
                sample = Some(s);
                break;
            }
        }
        let sample = sample.expect("Should have samples after tick");

        // Output should be clamped to i16::MAX (32767) for left
        // Output should be -1 for right (-32768 + 32767 = -1)
        assert_eq!(sample.0, 32767, "Left channel should clamp to maximum positive i16");
        assert_eq!(sample.1, -1, "Right channel should evaluate to -1");

        // Test underflow clamping
        // Clear buffers and add large negative deltas
        apu.fm.blip_l.clear();
        apu.fm.blip_r.clear();
        apu.psg.blip.clear();

        apu.fm.blip_l.add_delta(0, -32768);
        apu.fm.blip_r.add_delta(0, -32768);
        apu.psg.blip.add_delta(0, -32768);

        apu.tick_cycles(100);

        let mut sample2 = None;
        for _ in 0..100 {
            if let Some(s) = apu.generate_sample() {
                sample2 = Some(s);
                break;
            }
        }
        let sample2 = sample2.expect("Should have samples");

        // Output should be clamped to i16::MIN (-32768)
        assert_eq!(sample2.0, -32768, "Left channel should clamp to minimum negative i16");
        assert_eq!(sample2.1, -32768, "Right channel should clamp to minimum negative i16");
    }
}
