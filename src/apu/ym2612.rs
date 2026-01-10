//! Yamaha YM2612 FM Synthesis Chip
//!
//! The YM2612 provides 6 channels of FM synthesis, each with 4 operators.
//! It also supports DAC mode on channel 6 for PCM playback.
//!
//! ## Register Banks
//! - Bank 0 ($4000/$4001): Global registers + Channels 1-3
//! - Bank 1 ($4002/$4003): Channels 4-6
//!
//! ## Key Registers
//! - $22: LFO control
//! - $24-$26: Timer A/B
//! - $27: Channel 3 mode / Timer control
//! - $28: Key on/off
//! - $2A: DAC data
//! - $2B: DAC enable
//! - $30-$9F: Operator registers (per-channel, per-operator)
//! - $A0-$BF: Channel registers (frequency, algorithm, panning)

/// FM Operator (one of 4 per channel)
#[derive(Debug, Clone, Default)]
pub struct Operator {
    /// Detune (0-7)
    pub detune: u8,
    /// Frequency multiplier (0-15)
    pub multiply: u8,
    /// Total level / attenuation (0-127)
    pub total_level: u8,
    /// Key scale / rate scaling (0-3)
    pub rate_scale: u8,
    /// Attack rate (0-31)
    pub attack_rate: u8,
    /// AM enable
    pub am_enable: bool,
    /// Decay rate (0-31)
    pub decay_rate: u8,
    /// Sustain rate (0-31)
    pub sustain_rate: u8,
    /// Sustain level (0-15)
    pub sustain_level: u8,
    /// Release rate (0-15)
    pub release_rate: u8,
    /// SSG-EG mode
    pub ssg_eg: u8,
}

/// FM Channel (6 total)
#[derive(Debug, Clone, Default)]
pub struct Channel {
    /// Four operators per channel
    pub operators: [Operator; 4],
    /// Frequency number (11-bit, split across A0/A4)
    pub frequency: u16,
    /// Block/octave (3-bit)
    pub block: u8,
    /// Algorithm (0-7)
    pub algorithm: u8,
    /// Feedback (0-7)
    pub feedback: u8,
    /// Left output enable
    pub left_enable: bool,
    /// Right output enable
    pub right_enable: bool,
    /// Amplitude modulation sensitivity (0-3)
    pub ams: u8,
    /// Phase modulation sensitivity (0-7)
    pub pms: u8,
    /// Key on state per operator
    pub key_on: [bool; 4],
}

/// YM2612 FM Chip state
#[derive(Debug)]
pub struct Ym2612 {
    /// 6 FM channels
    pub channels: [Channel; 6],
    
    /// Address latch for each bank (0 and 1)
    address_latch: [u8; 2],
    
    // Global registers
    /// LFO enable
    pub lfo_enable: bool,
    /// LFO frequency (0-7)
    pub lfo_frequency: u8,
    
    /// Timer A value (10-bit)
    pub timer_a: u16,
    /// Timer B value (8-bit)
    pub timer_b: u8,
    /// Timer control register ($27)
    pub timer_control: u8,
    
    /// DAC enable (replaces channel 6)
    pub dac_enable: bool,
    /// DAC sample value
    pub dac_sample: u8,
    
    /// Busy flag (set briefly after writes)
    busy: bool,
    /// Timer A flag
    timer_a_flag: bool,
    /// Timer B flag
    timer_b_flag: bool,
}

impl Default for Ym2612 {
    fn default() -> Self {
        Self::new()
    }
}

impl Ym2612 {
    /// Create a new YM2612 in reset state
    pub fn new() -> Self {
        Self {
            channels: Default::default(),
            address_latch: [0; 2],
            lfo_enable: false,
            lfo_frequency: 0,
            timer_a: 0,
            timer_b: 0,
            timer_control: 0,
            dac_enable: false,
            dac_sample: 0,
            busy: false,
            timer_a_flag: false,
            timer_b_flag: false,
        }
    }
    
    /// Reset the chip
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    
    /// Write to address register
    pub fn write_address(&mut self, port: u8, value: u8) {
        self.address_latch[port as usize & 1] = value;
    }
    
    /// Write to data register
    pub fn write_data(&mut self, port: u8, value: u8) {
        let addr = self.address_latch[port as usize & 1];
        let bank = port & 1;
        
        self.write_register(bank, addr, value);
        self.busy = true; // Brief busy period after write
    }
    
    /// Read from chip (mainly status)
    pub fn read(&self, _port: u8) -> u8 {
        // Status register:
        // Bit 7: Busy flag
        // Bit 1: Timer B overflow
        // Bit 0: Timer A overflow
        let mut status = 0u8;
        if self.busy {
            status |= 0x80;
        }
        if self.timer_b_flag {
            status |= 0x02;
        }
        if self.timer_a_flag {
            status |= 0x01;
        }
        status
    }
    
    /// Write to a specific register
    fn write_register(&mut self, bank: u8, addr: u8, value: u8) {
        match addr {
            // Global registers (bank 0 only, but some accept on both)
            0x22 => {
                // LFO control
                self.lfo_enable = (value & 0x08) != 0;
                self.lfo_frequency = value & 0x07;
            }
            
            0x24 => {
                // Timer A MSB
                self.timer_a = (self.timer_a & 0x03) | ((value as u16) << 2);
            }
            
            0x25 => {
                // Timer A LSB (2 bits)
                self.timer_a = (self.timer_a & 0x3FC) | ((value as u16) & 0x03);
            }
            
            0x26 => {
                // Timer B
                self.timer_b = value;
            }
            
            0x27 => {
                // Timer control / Channel 3 mode
                self.timer_control = value;
                
                // Reset timer flags if requested
                if (value & 0x10) != 0 {
                    self.timer_a_flag = false;
                }
                if (value & 0x20) != 0 {
                    self.timer_b_flag = false;
                }
            }
            
            0x28 => {
                // Key on/off
                let channel = (value & 0x07) as usize;
                let channel_idx = if channel >= 4 { channel - 1 } else { channel };
                
                if channel_idx < 6 {
                    self.channels[channel_idx].key_on[0] = (value & 0x10) != 0;
                    self.channels[channel_idx].key_on[1] = (value & 0x20) != 0;
                    self.channels[channel_idx].key_on[2] = (value & 0x40) != 0;
                    self.channels[channel_idx].key_on[3] = (value & 0x80) != 0;
                }
            }
            
            0x2A => {
                // DAC data
                self.dac_sample = value;
            }
            
            0x2B => {
                // DAC enable
                self.dac_enable = (value & 0x80) != 0;
            }
            
            // Operator registers: $30-$9F
            0x30..=0x9F => {
                self.write_operator_register(bank, addr, value);
            }
            
            // Channel registers: $A0-$BF
            0xA0..=0xBF => {
                self.write_channel_register(bank, addr, value);
            }
            
            _ => {
                // Unknown/reserved register
            }
        }
    }
    
    /// Write to operator register
    fn write_operator_register(&mut self, bank: u8, addr: u8, value: u8) {
        // Operator registers are laid out as:
        // $3X: DT1/MUL
        // $4X: TL
        // $5X: RS/AR
        // $6X: AM/D1R
        // $7X: D2R
        // $8X: SL/RR
        // $9X: SSG-EG
        
        let reg_type = (addr >> 4) & 0x0F;
        let channel_in_bank = (addr & 0x03) as usize;
        let operator = ((addr >> 2) & 0x03) as usize;
        
        // Skip invalid channel indices
        if channel_in_bank >= 3 {
            return;
        }
        
        let channel_idx = channel_in_bank + (bank as usize * 3);
        if channel_idx >= 6 {
            return;
        }
        
        let op = &mut self.channels[channel_idx].operators[operator];
        
        match reg_type {
            0x3 => {
                // DT1 (bits 6-4), MUL (bits 3-0)
                op.detune = (value >> 4) & 0x07;
                op.multiply = value & 0x0F;
            }
            0x4 => {
                // TL (bits 6-0)
                op.total_level = value & 0x7F;
            }
            0x5 => {
                // RS (bits 7-6), AR (bits 4-0)
                op.rate_scale = (value >> 6) & 0x03;
                op.attack_rate = value & 0x1F;
            }
            0x6 => {
                // AM (bit 7), D1R (bits 4-0)
                op.am_enable = (value & 0x80) != 0;
                op.decay_rate = value & 0x1F;
            }
            0x7 => {
                // D2R (bits 4-0)
                op.sustain_rate = value & 0x1F;
            }
            0x8 => {
                // SL (bits 7-4), RR (bits 3-0)
                op.sustain_level = (value >> 4) & 0x0F;
                op.release_rate = value & 0x0F;
            }
            0x9 => {
                // SSG-EG (bits 3-0)
                op.ssg_eg = value & 0x0F;
            }
            _ => {}
        }
    }
    
    /// Write to channel register
    fn write_channel_register(&mut self, bank: u8, addr: u8, value: u8) {
        let _reg_type = (addr >> 2) & 0x07;
        let channel_in_bank = (addr & 0x03) as usize;
        
        if channel_in_bank >= 3 {
            return;
        }
        
        let channel_idx = channel_in_bank + (bank as usize * 3);
        if channel_idx >= 6 {
            return;
        }
        
        let ch = &mut self.channels[channel_idx];
        
        match addr & 0xFC {
            0xA0 => {
                // Frequency LSB
                ch.frequency = (ch.frequency & 0x700) | (value as u16);
            }
            0xA4 => {
                // Block (bits 5-3), Frequency MSB (bits 2-0)
                ch.block = (value >> 3) & 0x07;
                ch.frequency = (ch.frequency & 0x0FF) | (((value & 0x07) as u16) << 8);
            }
            0xB0 => {
                // Feedback (bits 5-3), Algorithm (bits 2-0)
                ch.feedback = (value >> 3) & 0x07;
                ch.algorithm = value & 0x07;
            }
            0xB4 => {
                // L (bit 7), R (bit 6), AMS (bits 5-4), PMS (bits 2-0)
                ch.left_enable = (value & 0x80) != 0;
                ch.right_enable = (value & 0x40) != 0;
                ch.ams = (value >> 4) & 0x03;
                ch.pms = value & 0x07;
            }
            _ => {}
        }
    }
    
    /// Step the chip (for future audio synthesis)
    pub fn step(&mut self) -> i16 {
        // Clear busy flag (it only lasts a few cycles)
        self.busy = false;
        
        // TODO: Actual FM synthesis
        // For now, just return DAC sample if enabled
        if self.dac_enable {
            // Convert unsigned 8-bit to signed 16-bit
            ((self.dac_sample as i16) - 128) << 8
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ym2612_new() {
        let ym = Ym2612::new();
        assert!(!ym.dac_enable);
        assert!(!ym.lfo_enable);
        assert_eq!(ym.timer_a, 0);
    }
    
    #[test]
    fn test_ym2612_dac_enable() {
        let mut ym = Ym2612::new();
        
        // Write to $2B to enable DAC
        ym.write_address(0, 0x2B);
        ym.write_data(0, 0x80);
        
        assert!(ym.dac_enable);
    }
    
    #[test]
    fn test_ym2612_key_on() {
        let mut ym = Ym2612::new();
        
        // Key on channel 0, all operators
        ym.write_address(0, 0x28);
        ym.write_data(0, 0xF0);  // All 4 operators on, channel 0
        
        assert!(ym.channels[0].key_on[0]);
        assert!(ym.channels[0].key_on[1]);
        assert!(ym.channels[0].key_on[2]);
        assert!(ym.channels[0].key_on[3]);
    }
    
    #[test]
    fn test_ym2612_lfo() {
        let mut ym = Ym2612::new();
        
        ym.write_address(0, 0x22);
        ym.write_data(0, 0x0F);  // LFO enable + max frequency
        
        assert!(ym.lfo_enable);
        assert_eq!(ym.lfo_frequency, 7);
    }
}
