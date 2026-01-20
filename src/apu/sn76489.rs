//! SN76489 Programmable Sound Generator (PSG)
//!
//! The SN76489 provides 3 square wave tone channels and 1 noise channel.
//! It is used for sound effects and backwards compatibility with Master System games.
//!
//! # Registers
//!
//! There are 8 internal registers:
//! - Tone 1 Frequency (10-bit)
//! - Tone 1 Attenuation (4-bit)
//! - Tone 2 Frequency (10-bit)
//! - Tone 2 Attenuation (4-bit)
//! - Tone 3 Frequency (10-bit)
//! - Tone 3 Attenuation (4-bit)
//! - Noise Control (3-bit)
//! - Noise Attenuation (4-bit)
//!
//! # Data Format
//!
//! Data is written as bytes.
//!
//! **Latch/Data Byte (Bit 7 = 1):**
//! `1 c c t d d d d`
//! - `cc`: Channel (00=Tone1, 01=Tone2, 10=Tone3, 11=Noise)
//! - `t`: Type (0=Frequency/Noise Control, 1=Attenuation)
//! - `dddd`: Data (Low 4 bits of frequency, or 4 bits of attenuation/noise control)
//!
//! **Data Byte (Bit 7 = 0):**
//! `0 - d d d d d d`
//! - `dddddd`: High 6 bits of frequency (for the currently latched tone channel).
//!   Note: Attenuation and Noise Control registers are fully set by the Latch byte
//!   and do not accept Data bytes.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Channel {
    Tone1 = 0,
    Tone2 = 1,
    Tone3 = 2,
    Noise = 3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegisterType {
    ToneNoise = 0,
    Volume = 1,
}

#[derive(Debug)]
pub struct Sn76489 {
    /// Tone 1 Frequency (10-bit)
    pub tone1_freq: u16,
    /// Tone 1 Attenuation (4-bit, 0=loudest, 15=silent)
    pub tone1_vol: u8,

    /// Tone 2 Frequency (10-bit)
    pub tone2_freq: u16,
    /// Tone 2 Attenuation
    pub tone2_vol: u8,

    /// Tone 3 Frequency (10-bit)
    pub tone3_freq: u16,
    /// Tone 3 Attenuation
    pub tone3_vol: u8,

    /// Noise Control (3-bit)
    /// Bit 2: Mode (0=Periodic, 1=White)
    /// Bits 0-1: Shift Rate (0=/16, 1=/32, 2=/64, 3=Tone3)
    pub noise_ctrl: u8,
    /// Noise Attenuation
    pub noise_vol: u8,

    /// Currently latched channel for Data byte updates
    latched_channel: Channel,
    /// Currently latched type (Frequency or Volume)
    latched_type: RegisterType,

    /// Configurable clock cycle count (internal emulator state)
    pub _cycles: u64,
}

impl Sn76489 {
    pub fn new() -> Self {
        Self {
            tone1_freq: 0,
            tone1_vol: 0x0F, // Silent
            tone2_freq: 0,
            tone2_vol: 0x0F,
            tone3_freq: 0,
            tone3_vol: 0x0F,
            noise_ctrl: 0,
            noise_vol: 0x0F,
            latched_channel: Channel::Tone1,
            latched_type: RegisterType::ToneNoise,
            _cycles: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Write a byte to the PSG port
    pub fn write(&mut self, data: u8) {
        if (data & 0x80) != 0 {
            // Latch/Data Byte
            let channel = match (data >> 5) & 0x03 {
                0 => Channel::Tone1,
                1 => Channel::Tone2,
                2 => Channel::Tone3,
                3 => Channel::Noise,
                _ => unreachable!(),
            };
            let type_bit = match (data >> 4) & 0x01 {
                0 => RegisterType::ToneNoise,
                1 => RegisterType::Volume,
                _ => unreachable!(),
            };
            let nibble = data & 0x0F;

            self.latched_channel = channel;
            self.latched_type = type_bit;

            match (channel, type_bit) {
                (Channel::Tone1, RegisterType::ToneNoise) => {
                    self.tone1_freq = (self.tone1_freq & 0xFFF0) | (nibble as u16);
                }
                (Channel::Tone1, RegisterType::Volume) => self.tone1_vol = nibble,
                (Channel::Tone2, RegisterType::ToneNoise) => {
                    self.tone2_freq = (self.tone2_freq & 0xFFF0) | (nibble as u16);
                }
                (Channel::Tone2, RegisterType::Volume) => self.tone2_vol = nibble,
                (Channel::Tone3, RegisterType::ToneNoise) => {
                    self.tone3_freq = (self.tone3_freq & 0xFFF0) | (nibble as u16);
                }
                (Channel::Tone3, RegisterType::Volume) => self.tone3_vol = nibble,
                (Channel::Noise, RegisterType::ToneNoise) => self.noise_ctrl = nibble & 0x07, // Only low 3 bits used
                (Channel::Noise, RegisterType::Volume) => self.noise_vol = nibble,
            }
        } else {
            // Data Byte
            // Applies only to Tone Frequency registers. Volume and Noise Ctrl are single-byte.
            // However, hardware behavior might allow updating whatever is latched.
            // Documentation usually says Data bytes only update frequency high 6 bits.
            // But some sources say it updates based on latched register.
            // For Tone Channels + Frequency type: It updates high bits.
            // For Volume or Noise: Data bytes are generally ignored or update low bits (uncommon).
            // Standard behavior: Data byte updates the high 6 bits of the *latched tone frequency*.

            if self.latched_type == RegisterType::ToneNoise && self.latched_channel != Channel::Noise {
                let value = (data & 0x3F) as u16;
                match self.latched_channel {
                    Channel::Tone1 => {
                        self.tone1_freq = (self.tone1_freq & 0x000F) | (value << 4);
                    }
                    Channel::Tone2 => {
                        self.tone2_freq = (self.tone2_freq & 0x000F) | (value << 4);
                    }
                    Channel::Tone3 => {
                        self.tone3_freq = (self.tone3_freq & 0x000F) | (value << 4);
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Default for Sn76489 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tone1_freq_update() {
        let mut psg = Sn76489::new();

        // 1. Latch Tone 1 Freq, with low 4 bits = 0xA
        // 1 00 0 1010 => 0x8A
        psg.write(0x8A);
        assert_eq!(psg.latched_channel, Channel::Tone1);
        assert_eq!(psg.latched_type, RegisterType::ToneNoise);
        assert_eq!(psg.tone1_freq, 0x0A);

        // 2. Write Data byte with high 6 bits = 0x15 (010101)
        // 0 010101 => 0x15
        psg.write(0x15);
        // Result should be 0x15A (0001 0101 1010)
        assert_eq!(psg.tone1_freq, 0x15A);
    }

    #[test]
    fn test_volume_update() {
        let mut psg = Sn76489::new();

        // Latch Tone 2 Volume, Data = 0x5
        // 1 01 1 0101 => 0xB5
        psg.write(0xB5);
        assert_eq!(psg.latched_channel, Channel::Tone2);
        assert_eq!(psg.latched_type, RegisterType::Volume);
        assert_eq!(psg.tone2_vol, 0x05);

        // Data byte should be ignored for volume (standard behavior)
        psg.write(0x20);
        assert_eq!(psg.tone2_vol, 0x05); // Unchanged
    }

    #[test]
    fn test_noise_control() {
        let mut psg = Sn76489::new();

        // Latch Noise Control (Type=0), Data = 0x6 (White Noise, rate /64)
        // 1 11 0 0110 => 0xE6
        psg.write(0xE6);
        assert_eq!(psg.latched_channel, Channel::Noise);
        assert_eq!(psg.noise_ctrl, 0x06); // Bit 2=1 (White), Bits 1-0=2 (Shift)
    }
}
