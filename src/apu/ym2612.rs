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
    /// Bit 2: Timer B overflow
    /// Bit 1: Timer A overflow
    pub status: u8,

    /// Internal timer counter (skeletal)
    timer_a_counter: u32,
}

impl Ym2612 {
    pub fn new() -> Self {
        Self {
            registers: [[0; 256]; 2],
            addr0: 0,
            addr1: 0,
            status: 0,
            timer_a_counter: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read Status Register
    pub fn read_status(&self) -> u8 {
        // In a real implementation, busy flag depends on write timing.
        // For now, always return status with some timer flags if they are enabled
        // to prevent sound drivers from hanging.
        self.status
    }

    /// Update timers based on elapsed cycles (skeletal)
    pub fn step(&mut self, _cycles: u32) {
        // TODO: Proper timer implementation. 
        // For now, we'll just toggle the timer A overflow bit occasionally 
        // if it's enabled (Reg 0x27 bit 0) to keep drivers moving.
        if (self.registers[0][0x27] & 0x01) != 0 {
             self.timer_a_counter += 1;
             if self.timer_a_counter > 100 {
                 self.status |= 0x01; // Timer A overflow
                 self.timer_a_counter = 0;
             }
        }
        if (self.registers[0][0x27] & 0x02) != 0 {
             self.status |= 0x02; // Timer B overflow
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
        self.registers[0][self.addr0 as usize] = val;
        // Handle global registers or immediate actions if necessary
    }

    /// Write to Address Port 1 (Part II)
    pub fn write_addr1(&mut self, val: u8) {
        self.addr1 = val;
    }

    /// Write to Data Port 1 (Part II)
    pub fn write_data1(&mut self, val: u8) {
        self.registers[1][self.addr1 as usize] = val;
    }

    // === Helper Accessors ===

    /// Get frequency block and f-number for a channel (0-2 for Bank0, 3-5 for Bank1)
    pub fn get_frequency(&self, channel: usize) -> (u8, u16) {
        let (bank, offset) = if channel < 3 { (0, channel) } else { (1, channel - 3) };
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
    fn test_timer_a_increment() {
        let mut ym = Ym2612::new();

        // Enable Timer A (Reg 0x27 bit 0)
        ym.write_addr0(0x27);
        ym.write_data0(0x01);

        assert_eq!(ym.timer_a_counter, 0, "Timer A counter should start at 0");

        ym.step(10);

        assert_eq!(ym.timer_a_counter, 1, "Timer A counter should increment after step");
        assert_eq!(ym.status & 0x01, 0, "Timer A overflow flag should not be set yet");
    }

    #[test]
    fn test_timer_a_overflow() {
        let mut ym = Ym2612::new();

        // Enable Timer A
        ym.write_addr0(0x27);
        ym.write_data0(0x01);

        // Step enough times to overflow (threshold is > 100)
        for _ in 0..101 {
            ym.step(10);
        }

        assert_eq!(ym.status & 0x01, 0x01, "Timer A overflow flag should be set");
        assert_eq!(ym.timer_a_counter, 0, "Timer A counter should reset after overflow");
    }

    #[test]
    fn test_timer_b_basic() {
        let mut ym = Ym2612::new();

        // Enable Timer B (Reg 0x27 bit 1)
        ym.write_addr0(0x27);
        ym.write_data0(0x02);

        ym.step(10);

        assert_eq!(ym.status & 0x02, 0x02, "Timer B overflow flag should be set immediately");
    }
}
