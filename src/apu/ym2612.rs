//! Yamaha YM2612 FM Synthesizer
//!
//! The YM2612 provides 6 channels of FM synthesis, with 4 operators per channel.
//! It also supports a DAC sample channel (replacing channel 6).
//!
//! # Registers
//!
//! Registers are split into two banks:
//! - Bank 0: 0x00-0x9F (Controls Channels 1-3)
//! - Bank 1: 0x00-0x9F (Controls Channels 4-6)
//!
//! Each channel has registers for:
//! - Phase Generator (Frequency)
//! - Envelope Generator (Attack, Decay, Sustain, Release)
//! - LFO
//! - Feedback/Algorithm
//!

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Sine table size (must be power of 2 for masking)
const SINE_TABLE_SIZE: usize = 1024;

/// Precomputed sine table for fast lookup
static SINE_TABLE: OnceLock<[f32; SINE_TABLE_SIZE]> = OnceLock::new();

fn get_sine_table() -> &'static [f32; SINE_TABLE_SIZE] {
    SINE_TABLE.get_or_init(|| {
        let mut table = [0.0; SINE_TABLE_SIZE];
        for i in 0..SINE_TABLE_SIZE {
            table[i] = (i as f32 * 2.0 * std::f32::consts::PI / SINE_TABLE_SIZE as f32).sin();
        }
        table
    })
}

mod register_array {
    use crate::memory::byte_utils::big_array;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(data: &[[u8; 256]; 2], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeTuple;
        let mut s = serializer.serialize_tuple(2)?;
        // We can't use big_array::serialize directly because it expects serializer, not reference.
        // We define a wrapper struct locally and serialize that.
        #[derive(Serialize)]
        struct Wrapper<'a>(#[serde(with = "big_array")] &'a [u8; 256]);

        s.serialize_element(&Wrapper(&data[0]))?;
        s.serialize_element(&Wrapper(&data[1]))?;
        s.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[[u8; 256]; 2], D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(with = "big_array")] [u8; 256]);

        let arr: [Wrapper; 2] = Deserialize::deserialize(deserializer)?;
        Ok([arr[0].0, arr[1].0])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Bank {
    Bank0 = 0,
    Bank1 = 1,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ym2612 {
    /// Internal registers (split into two banks of 256 bytes for simplicity,
    /// though many are unused).
    /// Bank 0: [0][addr]
    /// Bank 1: [1][addr]
    #[serde(with = "register_array")]
    pub registers: [[u8; 256]; 2],

    /// Current register addresses for each bank
    address: [u8; 2],

    /// Status register
    /// Bit 7: Busy
    /// Bit 1: Timer B overflow
    /// Bit 0: Timer A overflow
    pub status: u8,

    /// Timer A counter (counts down, Master Cycles)
    timer_a_count: i32,
    /// Timer B counter (counts down, Master Cycles)
    timer_b_count: i32,

    /// Busy flag counter (counts down, Master Cycles)
    busy_cycles: i32,

    /// Phase accumulators for the 6 channels (simplified FM)
    phase: [f32; 6],
    /// Phase increment for the 6 channels (cached)
    phase_inc: [f32; 6],
    /// DAC value (register 0x2A)
    dac_value: u8,
    /// DAC enabled (register 0x2B bit 7)
    dac_enabled: bool,
}

impl Ym2612 {
    pub fn new() -> Self {
        Self {
            registers: [[0; 256]; 2],
            address: [0; 2],
            status: 0,
            timer_a_count: 0,
            timer_b_count: 0,
            busy_cycles: 0,
            phase: [0.0; 6],
            phase_inc: [0.0; 6],
            dac_value: 0x80,
            dac_enabled: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read Status Register
    pub fn read_status(&self) -> u8 {
        // Return status with timer flags and busy flag
        if self.busy_cycles > 0 {
            self.status | 0x80
        } else {
            self.status
        }
    }

    /// Update timers based on elapsed M68k cycles
    pub fn step(&mut self, cycles: u32) {
        // Convert M68k cycles to Master Cycles (x7)
        let cycles = (cycles * 7) as i32;

        if self.busy_cycles > 0 {
            self.busy_cycles -= cycles;
        }

        let ctrl = self.registers[0][0x27];

        // Timer A
        // Bit 0: Load A (Enable Counting)
        if (ctrl & 0x01) != 0 {
            self.timer_a_count -= cycles;
            if self.timer_a_count <= 0 {
                // Calculate period: (1024 - N) * 144
                // N = (Reg 0x24 << 2) | (Reg 0x25 & 0x03)
                let n = ((self.registers[0][0x24] as u32) << 2)
                    | (self.registers[0][0x25] as u32 & 0x03);
                let period = (1024 - n as i32) * 144;

                // If period is 0 or very small, force minimum to avoid infinite loops
                let period = if period < 144 { 144 } else { period };

                while self.timer_a_count <= 0 {
                    self.timer_a_count += period;
                    // Bit 2: Enable A (Flag)
                    if (ctrl & 0x04) != 0 {
                        self.status |= 0x01; // Set Timer A Overflow (Bit 0)
                    }
                }
            }
        }

        // Timer B
        // Bit 1: Load B (Enable Counting)
        if (ctrl & 0x02) != 0 {
            self.timer_b_count -= cycles;
            if self.timer_b_count <= 0 {
                // Calculate period: (256 - N) * 2304
                // N = Reg 0x26
                let n = self.registers[0][0x26] as u32;
                let period = (256 - n as i32) * 2304;

                // Minimum period check
                let period = if period < 2304 { 2304 } else { period };

                while self.timer_b_count <= 0 {
                    self.timer_b_count += period;
                    // Bit 3: Enable B (Flag)
                    if (ctrl & 0x08) != 0 {
                        self.status |= 0x02; // Set Timer B Overflow (Bit 1)
                    }
                }
            }
        }
    }

    /// Unified read from port (0 or 1)
    pub fn read(&self, _port: u8) -> u8 {
        self.read_status()
    }

    /// Unified write address to port (0 or 1)
    pub fn write_address(&mut self, port: u8, val: u8) {
        let bank = if port == 0 { Bank::Bank0 } else { Bank::Bank1 };
        self.write_addr(bank, val);
    }

    /// Unified write data to port (0 or 1)
    pub fn write_data(&mut self, port: u8, val: u8) {
        let bank = if port == 0 { Bank::Bank0 } else { Bank::Bank1 };
        self.write_data_bank(bank, val);
    }

    /// Write to address port for a specific bank
    pub fn write_addr(&mut self, bank: Bank, val: u8) {
        self.address[bank as usize] = val;
    }

    /// Write to data port for a specific bank
    pub fn write_data_bank(&mut self, bank: Bank, val: u8) {
        // Set busy flag duration (32 internal YM2612 cycles * 7 = 224 Master Cycles)
        // This corresponds to 32 M68k cycles.
        self.busy_cycles = 224;

        let bank_idx = bank as usize;
        let addr = self.address[bank_idx];

        match (bank, addr) {
            (Bank::Bank0, 0x27) => self.handle_timer_control(val),
            (Bank::Bank0, 0x2A) => self.handle_dac_data(val),
            (Bank::Bank0, 0x2B) => self.handle_dac_enable(val),
            _ => {
                self.registers[bank_idx][addr as usize] = val;
                // Update cached phase increment if frequency registers are written
                if (0xA0..=0xA2).contains(&addr) || (0xA4..=0xA6).contains(&addr) {
                    let ch_offset = (addr & 0x03) as usize;
                    let ch = if bank == Bank::Bank0 {
                        ch_offset
                    } else {
                        ch_offset + 3
                    };
                    self.update_phase_inc(ch);
                }
            }
        }
    }

    fn handle_timer_control(&mut self, val: u8) {
        let old_val = self.registers[0][0x27];

        // Handle Reset Flags
        // Bit 4: Reset A (Clear Timer A Overflow)
        if (val & 0x10) != 0 {
            self.status &= !0x01;
        }
        // Bit 5: Reset B (Clear Timer B Overflow)
        if (val & 0x20) != 0 {
            self.status &= !0x02;
        }

        // Handle Load Transitions (Reload Counters)
        // Load A (Bit 0) 0->1
        if (val & 0x01) != 0 && (old_val & 0x01) == 0 {
            let n = ((self.registers[0][0x24] as u32) << 2)
                | (self.registers[0][0x25] as u32 & 0x03);
            let period = (1024 - n as i32) * 144;
            self.timer_a_count = if period < 144 { 144 } else { period };
        }

        // Load B (Bit 1) 0->1
        if (val & 0x02) != 0 && (old_val & 0x02) == 0 {
            let n = self.registers[0][0x26] as u32;
            let period = (256 - n as i32) * 2304;
            self.timer_b_count = if period < 2304 { 2304 } else { period };
        }

        self.registers[0][0x27] = val;
    }

    fn handle_dac_data(&mut self, val: u8) {
        self.dac_value = val;
        self.registers[0][0x2A] = val;
    }

    fn handle_dac_enable(&mut self, val: u8) {
        self.dac_enabled = (val & 0x80) != 0;
        self.registers[0][0x2B] = val;
    }

    /// Generate one stereo sample pair
    pub fn generate_sample(&mut self) -> (i16, i16) {
        let mut left: f32 = 0.0;
        let mut right: f32 = 0.0;
        let sine_table = get_sine_table();

        // Channel 6 DAC mode
        if self.dac_enabled {
            // DAC value is unsigned 8-bit, convert to centered float
            let dac_f = (self.dac_value as f32 - 128.0) / 128.0;

            // Panning for Channel 6
            let pan = self.registers[1][0xB6];
            if (pan & 0x80) != 0 {
                left += dac_f;
            }
            if (pan & 0x40) != 0 {
                right += dac_f;
            }
        }

        // Channels 1-6 (including Ch 6 if DAC is off)
        for ch in 0..6 {
            if ch == 5 && self.dac_enabled {
                continue;
            }

            let inc = self.phase_inc[ch];
            if inc == 0.0 {
                continue;
            }

            self.phase[ch] = (self.phase[ch] + inc) % 1.0;

            // Table-based sine wave lookup
            let table_idx =
                (self.phase[ch] * SINE_TABLE_SIZE as f32) as usize & (SINE_TABLE_SIZE - 1);
            let sample_val = sine_table[table_idx];

            let (bank, offset) = if ch < 3 { (0, ch) } else { (1, ch - 3) };
            let tl = self.registers[bank][0x4C + offset] & 0x7F;
            let volume = (127.0 - tl as f32) / 127.0;

            let sample_out = sample_val * volume * 0.2;

            // Optimized Panning
            let pan_addr = 0xB4 + (ch % 3);
            let pan = self.registers[if ch < 3 { 0 } else { 1 }][pan_addr];
            if (pan & 0x80) != 0 || pan == 0 {
                left += sample_out;
            }
            if (pan & 0x40) != 0 || pan == 0 {
                right += sample_out;
            }
        }

        // Clamp and convert to i16
        (
            (left.clamp(-1.0, 1.0) * 16384.0) as i16,
            (right.clamp(-1.0, 1.0) * 16384.0) as i16,
        )
    }

    // === Helper Accessors ===

    /// Get frequency block and f-number for a channel (0-2 for Bank0, 3-5 for Bank1)
    pub fn get_frequency(&self, channel: usize) -> (u8, u16) {
        let (bank, offset) = if channel < 3 {
            (0, channel)
        } else {
            (1, channel - 3)
        };
        let addr_hi = 0xA4 + offset;
        let addr_lo = 0xA0 + offset;

        let hi = self.registers[bank][addr_hi];
        let lo = self.registers[bank][addr_lo];

        let block = (hi >> 3) & 0x07;
        let f_num = ((hi as u16 & 0x07) << 8) | (lo as u16);
        (block, f_num)
    }

    /// Check if channel key is on (conceptually, exact register is per-operator)
    /// Key On is actually handled via register 0x28 in Bank 0
    pub fn is_key_on(&self) -> u8 {
        // Register 0x28: Slot/Key On
        // This is a write-only trigger usually, but we store it?
        // Actually YM2612 doesn't store "Key On" state in a readable register necessarily,
        // it updates the operators. We'd need internal state for that.
        // For this skeletal implementation, we'll just check if we stored the last write.
        // But 0x28 is in Bank 0 and applies to all channels based on bits 0-2 (channel) and 4-7 (slots).
        self.registers[0][0x28]
    }

    fn update_phase_inc(&mut self, ch: usize) {
        let (block, f_num) = self.get_frequency(ch);
        if f_num == 0 {
            self.phase_inc[ch] = 0.0;
        } else {
            // Internal YM2612 frequency formula:
            // F_internal = (clock / (144 * 2^2)) * (f_num / 2^10) * 2^block
            // For a 7.67MHz clock, this gives approx 53kHz max frequency.
            // We need the increment per output sample (44.1kHz).
            
            let freq_mult = (1 << block) as f32;
            // (f_num * freq_mult) / (2^10 * 144) is the frequency in internal ticks
            // But we simplify: YM2612 clock is 7670453 Hz.
            // One sample is 144 clock cycles.
            // phase_inc = (f_num * 2^(block-1)) / (2^11 * 44100 / 53267) -- roughly.
            // Let's use a more standard approach: 
            // inc = (F_internal) / F_sample_rate
            let f_internal = (f_num as f32 * freq_mult * 7670453.0) / (144.0 * 1024.0 * 8.0);
            self.phase_inc[ch] = f_internal / 44100.0;
        }
    }
}

impl Default for Ym2612 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bank_select() {
        let mut ym = Ym2612::new();

        // Write to Bank 0, Register 0x30 (Detune/Mult for Ch1 Op1)
        ym.write_addr(Bank::Bank0, 0x30);
        ym.write_data_bank(Bank::Bank0, 0x71);

        assert_eq!(ym.registers[0][0x30], 0x71);
        assert_eq!(ym.registers[1][0x30], 0x00);

        // Write to Bank 1, Register 0x30 (Detune/Mult for Ch4 Op1)
        ym.write_addr(Bank::Bank1, 0x30);
        ym.write_data_bank(Bank::Bank1, 0x42);

        assert_eq!(ym.registers[1][0x30], 0x42);
        assert_eq!(ym.registers[0][0x30], 0x71);
    }

    #[test]
    fn test_frequency_setting_bank1() {
        let mut ym = Ym2612::new();

        // Set Ch4 Frequency (Bank 1, Channel 0)
        // This corresponds to channel index 3 in get_frequency.
        // Bank 1 registers are accessed via port 1.
        // F-Num low = 0x55 (Reg 0xA0)
        // Block/F-Num high = 0x22 (Reg 0xA4) -> Block 4, F-Num high 2
        ym.write_addr(Bank::Bank1, 0xA0);
        ym.write_data_bank(Bank::Bank1, 0x55);
        ym.write_addr(Bank::Bank1, 0xA4);
        ym.write_data_bank(Bank::Bank1, 0x22); // 001 00010 (Block 4, Hi 2)

        let (block, f_num) = ym.get_frequency(3); // Channel 3 is first channel of Bank 1
                                                  // Reg 0xA4 = 0x22 = 0010 0010. Bits 5-3 are Block (100 = 4). Bits 2-0 are F-High (010 = 2).
        assert_eq!(block, 4);
        assert_eq!(f_num, 0x255); // 0x200 | 0x55

        // Verify isolation: Bank 0 Channel 0 (index 0) should be 0
        let (block0, f_num0) = ym.get_frequency(0);
        assert_eq!(block0, 0);
        assert_eq!(f_num0, 0);
    }

    #[test]
    fn test_frequency_setting() {
        let mut ym = Ym2612::new();

        // Set Ch1 Frequency (Bank 0)
        // F-Num low = 0x55 (Reg 0xA0)
        // Block/F-Num high = 0x22 (Reg 0xA4) -> Block 4, F-Num high 2
        ym.write_addr(Bank::Bank0, 0xA0);
        ym.write_data_bank(Bank::Bank0, 0x55);
        ym.write_addr(Bank::Bank0, 0xA4);
        ym.write_data_bank(Bank::Bank0, 0x22); // 001 00010 (Block 4, Hi 2)

        let (block, f_num) = ym.get_frequency(0);
        // Reg 0xA4 = 0x22 = 0010 0010. Bits 5-3 are Block (100 = 4). Bits 2-0 are F-High (010 = 2).
        assert_eq!(block, 4);
        assert_eq!(f_num, 0x255); // 0x200 | 0x55
    }

    #[test]
    fn test_timer_a() {
        let mut ym = Ym2612::new();

        // Configure Timer A
        // N = 1000. Period = (1024 - 1000) * 144 = 24 * 144 = 3456 Master Cycles.
        // Reg 0x24 (High 8 bits) = 1000 >> 2 = 250 (0xFA)
        // Reg 0x25 (Low 2 bits) = 1000 & 3 = 0
        ym.write_addr(Bank::Bank0, 0x24);
        ym.write_data_bank(Bank::Bank0, 0xFA);
        ym.write_addr(Bank::Bank0, 0x25);
        ym.write_data_bank(Bank::Bank0, 0x00);

        // Enable Timer A (Bit 0) and Enable Flag (Bit 2) -> 0x05
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x05);

        assert_eq!(ym.timer_a_count, 3456);

        // Step. Need 3456 Master Cycles.
        // Step takes 68k cycles. 1 68k = 7 Master.
        // Need 3456 / 7 = 493.7 68k cycles.

        ym.step(493);
        assert_eq!(ym.status & 0x01, 0, "Timer A should not have fired yet");

        ym.step(1); // Total 494 * 7 = 3458 > 3456
        assert_eq!(ym.status & 0x01, 0x01, "Timer A should have fired");

        // Reset Flag
        // Write 0x05 | 0x10 (Reset Flag A) = 0x15
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x15);
        assert_eq!(ym.status & 0x01, 0, "Timer A flag should be cleared");

        // Wait for next overflow
        ym.step(494);
        assert_eq!(ym.status & 0x01, 0x01, "Timer A should fire again");
    }

    #[test]
    fn test_timer_b() {
        let mut ym = Ym2612::new();

        // Configure Timer B
        // N = 200. Period = (256 - 200) * 2304 = 56 * 2304 = 129024 Master Cycles.
        // Reg 0x26 = 200 (0xC8)
        ym.write_addr(Bank::Bank0, 0x26);
        ym.write_data_bank(Bank::Bank0, 0xC8);

        // Enable Timer B (Bit 1) and Enable Flag (Bit 3) -> 0x0A
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x0A);

        assert_eq!(ym.timer_b_count, 129024);

        // Need 129024 / 7 = 18432.

        ym.step(18431);
        assert_eq!(ym.status & 0x02, 0, "Timer B should not have fired yet");

        ym.step(2);
        assert_eq!(ym.status & 0x02, 0x02, "Timer B should have fired");

        // Reset Flag
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x0A | 0x20); // 0x2A
        assert_eq!(ym.status & 0x02, 0, "Timer B flag should be cleared");
    }

    #[test]
    fn test_timer_reset_flags() {
        let mut ym = Ym2612::new();
        ym.status = 0x03; // Both flags set

        // Reset A (Bit 4) -> 0x10
        // Preserve Load/Enable bits if we wanted, but writing 0x10 disables them unless we set them too.
        // Actually writing 0x10 sets Load=0, Enable=0. So it stops timers too.
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x10);

        assert_eq!(ym.status & 0x01, 0x00); // A cleared
        assert_eq!(ym.status & 0x02, 0x02); // B stays

        // Reset B (Bit 5) -> 0x20
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x20);
        assert_eq!(ym.status & 0x02, 0x00); // B cleared
    }

    #[test]
    fn test_timer_load_restart() {
        let mut ym = Ym2612::new();

        // N = 1023. Period 144. (20.5 68k cycles)
        ym.write_addr(Bank::Bank0, 0x24);
        ym.write_data_bank(Bank::Bank0, 0xFF);
        ym.write_addr(Bank::Bank0, 0x25);
        ym.write_data_bank(Bank::Bank0, 0x03);

        // Enable Timer A with Flag (0x05)
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x05);

        ym.step(15); // 105 master cycles.

        // Stop (0x04 - keep flag enabled but stop timer? or 0x00)
        // Write 0x04 (Flag enable only, Load=0).
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x04);

        // Start (0x05). 0->1 transition on Bit 0. Reloads.
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x05);

        ym.step(15); // 105 master. Total 105 (if reloaded).
        assert_eq!(ym.status & 0x01, 0, "Should have reloaded and not fired");

        ym.step(10); // +70 = 175. Fire.
        assert_eq!(ym.status & 0x01, 0x01);
    }

    #[test]
    fn test_frequency_setting_bank1_offset1() {
        let mut ym = Ym2612::new();
        // This corresponds to channel index 4 in get_frequency.
        // Registers are in Bank 1.
        // Base for Bank 1 (offset 1) is:
        // Low: 0xA0 + 1 = 0xA1
        // High: 0xA4 + 1 = 0xA5

        // Write Low byte 0x55 to 0xA1 (Bank 1)
        ym.write_addr(Bank::Bank1, 0xA1);
        ym.write_data_bank(Bank::Bank1, 0x55);

        // Write High byte 0x22 to 0xA5 (Bank 1) -> Block 4, F-Num High 2
        ym.write_addr(Bank::Bank1, 0xA5);
        ym.write_data_bank(Bank::Bank1, 0x22);

        let (block, f_num) = ym.get_frequency(4);
        assert_eq!(block, 4);
        assert_eq!(f_num, 0x255);

        // Ensure Bank 0 (Channel 1, offset 1) is unaffected.
        // Channel 1 corresponds to index 1.
        // Registers 0xA1, 0xA5 in Bank 0.
        let (block0, f_num0) = ym.get_frequency(1);
        assert_eq!(block0, 0);
        assert_eq!(f_num0, 0);
    }

    #[test]
    fn test_busy_flag() {
        let mut ym = Ym2612::new();

        // Initially not busy
        assert_eq!(ym.read_status() & 0x80, 0);

        // Write to Data Port (any value)
        ym.write_data(0, 0x00);

        // Should be busy immediately
        assert_eq!(ym.read_status() & 0x80, 0x80);

        // Step for 31 68k cycles (31 * 7 = 217 Master Cycles)
        ym.step(31);
        assert_eq!(
            ym.read_status() & 0x80,
            0x80,
            "Should still be busy at 31 cycles"
        );

        // Step 1 more cycle (total 32 * 7 = 224)
        ym.step(1);
        // busy_cycles -= 7 -> 0.
        assert_eq!(ym.read_status() & 0x80, 0, "Should be free after 32 cycles");
    }

    #[test]
    fn test_sample_generation_basic() {
        let mut ym = Ym2612::new();

        // Set Ch1 Frequency
        ym.write_addr(Bank::Bank0, 0xA0);
        ym.write_data_bank(Bank::Bank0, 0x55);
        ym.write_addr(Bank::Bank0, 0xA4);
        ym.write_data_bank(Bank::Bank0, 0x22);

        // Set Ch1 Volume (TL) to 0 (Max)
        ym.write_addr(Bank::Bank0, 0x4C); // Op 4 TL
        ym.write_data_bank(Bank::Bank0, 0x00);

        // Generate some samples
        let mut saw_non_zero = false;
        for _ in 0..100 {
            let (l, r) = ym.generate_sample();
            if l != 0 || r != 0 {
                saw_non_zero = true;
                break;
            }
        }
        assert!(
            saw_non_zero,
            "Should generate non-zero samples when channel is active"
        );
    }
}
