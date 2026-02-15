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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bank {
    Bank0 = 0,
    Bank1 = 1,
}

#[derive(Debug)]
pub struct Ym2612 {
    /// Internal registers (split into two banks of 256 bytes for simplicity,
    /// though many are unused).
    /// Bank 0: [0][addr]
    /// Bank 1: [1][addr]
    pub registers: [[u8; 256]; 2],

    /// Current register address for Bank 0 port
    addr0: u8,
    /// Current register address for Bank 1 port
    addr1: u8,

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
}

impl Ym2612 {
    pub fn new() -> Self {
        Self {
            registers: [[0; 256]; 2],
            addr0: 0,
            addr1: 0,
            status: 0,
            timer_a_count: 0,
            timer_b_count: 0,
            busy_cycles: 0,
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
        if port == 0 {
            self.write_addr0(val);
        } else {
            self.write_addr1(val);
        }
    }

    /// Unified write data to port (0 or 1)
    pub fn write_data(&mut self, port: u8, val: u8) {
        if port == 0 {
            self.write_data0(val);
        } else {
            self.write_data1(val);
        }
    }

    /// Write to Address Port 0 (Part I)
    pub fn write_addr0(&mut self, val: u8) {
        self.addr0 = val;
    }

    /// Write to Data Port 0 (Part I)
    pub fn write_data0(&mut self, val: u8) {
        // Set busy flag duration (32 internal YM2612 cycles * 7 = 224 Master Cycles)
        // This corresponds to 32 M68k cycles.
        self.busy_cycles = 224;

        if self.addr0 == 0x27 {
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
        } else {
            self.registers[0][self.addr0 as usize] = val;
        }
    }

    /// Write to Address Port 1 (Part II)
    pub fn write_addr1(&mut self, val: u8) {
        self.addr1 = val;
    }

    /// Write to Data Port 1 (Part II)
    pub fn write_data1(&mut self, val: u8) {
        self.busy_cycles = 224;
        self.registers[1][self.addr1 as usize] = val;
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
        ym.write_addr0(0x30);
        ym.write_data0(0x71);

        assert_eq!(ym.registers[0][0x30], 0x71);
        assert_eq!(ym.registers[1][0x30], 0x00);

        // Write to Bank 1, Register 0x30 (Detune/Mult for Ch4 Op1)
        ym.write_addr1(0x30);
        ym.write_data1(0x42);

        assert_eq!(ym.registers[1][0x30], 0x42);
        assert_eq!(ym.registers[0][0x30], 0x71);
    }

    #[test]
    fn test_frequency_setting() {
        let mut ym = Ym2612::new();

        // Set Ch1 Frequency (Bank 0)
        // F-Num low = 0x55 (Reg 0xA0)
        // Block/F-Num high = 0x22 (Reg 0xA4) -> Block 4, F-Num high 2
        ym.write_addr0(0xA0);
        ym.write_data0(0x55);
        ym.write_addr0(0xA4);
        ym.write_data0(0x22); // 001 00010 (Block 4, Hi 2)

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
        ym.write_addr0(0x24);
        ym.write_data0(0xFA);
        ym.write_addr0(0x25);
        ym.write_data0(0x00);

        // Enable Timer A (Bit 0) and Enable Flag (Bit 2) -> 0x05
        ym.write_addr0(0x27);
        ym.write_data0(0x05);

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
        ym.write_addr0(0x27);
        ym.write_data0(0x15);
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
        ym.write_addr0(0x26);
        ym.write_data0(0xC8);

        // Enable Timer B (Bit 1) and Enable Flag (Bit 3) -> 0x0A
        ym.write_addr0(0x27);
        ym.write_data0(0x0A);

        assert_eq!(ym.timer_b_count, 129024);

        // Need 129024 / 7 = 18432.

        ym.step(18431);
        assert_eq!(ym.status & 0x02, 0, "Timer B should not have fired yet");

        ym.step(2);
        assert_eq!(ym.status & 0x02, 0x02, "Timer B should have fired");

        // Reset Flag
        ym.write_addr0(0x27);
        ym.write_data0(0x0A | 0x20); // 0x2A
        assert_eq!(ym.status & 0x02, 0, "Timer B flag should be cleared");
    }

    #[test]
    fn test_timer_reset_flags() {
        let mut ym = Ym2612::new();
        ym.status = 0x03; // Both flags set

        // Reset A (Bit 4) -> 0x10
        // Preserve Load/Enable bits if we wanted, but writing 0x10 disables them unless we set them too.
        // Actually writing 0x10 sets Load=0, Enable=0. So it stops timers too.
        ym.write_addr0(0x27);
        ym.write_data0(0x10);

        assert_eq!(ym.status & 0x01, 0x00); // A cleared
        assert_eq!(ym.status & 0x02, 0x02); // B stays

        // Reset B (Bit 5) -> 0x20
        ym.write_addr0(0x27);
        ym.write_data0(0x20);
        assert_eq!(ym.status & 0x02, 0x00); // B cleared
    }

    #[test]
    fn test_timer_load_restart() {
        let mut ym = Ym2612::new();

        // N = 1023. Period 144. (20.5 68k cycles)
        ym.write_addr0(0x24);
        ym.write_data0(0xFF);
        ym.write_addr0(0x25);
        ym.write_data0(0x03);

        // Enable Timer A with Flag (0x05)
        ym.write_addr0(0x27);
        ym.write_data0(0x05);

        ym.step(15); // 105 master cycles.

        // Stop (0x04 - keep flag enabled but stop timer? or 0x00)
        // Write 0x04 (Flag enable only, Load=0).
        ym.write_addr0(0x27);
        ym.write_data0(0x04);

        // Start (0x05). 0->1 transition on Bit 0. Reloads.
        ym.write_addr0(0x27);
        ym.write_data0(0x05);

        ym.step(15); // 105 master. Total 105 (if reloaded).
        assert_eq!(ym.status & 0x01, 0, "Should have reloaded and not fired");

        ym.step(10); // +70 = 175. Fire.
        assert_eq!(ym.status & 0x01, 0x01);
    }

    #[test]
    fn test_frequency_setting_bank1() {
        let mut ym = Ym2612::new();

        // Set Ch4 Frequency (Bank 1, offset 1)
        // This corresponds to channel index 4 in get_frequency.
        // Registers are in Bank 1.
        // Base for Bank 1 (offset 1) is:
        // Low: 0xA0 + 1 = 0xA1
        // High: 0xA4 + 1 = 0xA5

        // Write Low byte 0x55 to 0xA1 (Bank 1)
        ym.write_addr1(0xA1);
        ym.write_data1(0x55);

        // Write High byte 0x22 to 0xA5 (Bank 1) -> Block 4, F-Num High 2
        ym.write_addr1(0xA5);
        ym.write_data1(0x22);

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
}
