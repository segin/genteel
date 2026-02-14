//! Texas Instruments SN76489 Programmable Sound Generator (PSG)
//!
//! The SN76489 provides:
//! - 3 square wave tone channels
//! - 1 noise channel (white or periodic)
//! - 4-bit volume control per channel
//!
//! ## Register Format
//! First byte has bit 7 set and contains:
//! - Bits 6-5: Channel (0-3)
//! - Bit 4: Type (0 = frequency, 1 = volume)
//! - Bits 3-0: Data
//!
//! Second byte (for frequency) has bit 7 clear and contains:
//! - Bits 5-0: Additional frequency data

/// Square wave tone channel
#[derive(Debug, Clone, Default)]
pub struct ToneChannel {
    /// 10-bit frequency divider (higher = lower pitch)
    pub frequency: u16,
    /// 4-bit volume (0 = max, 15 = off)
    pub volume: u8,
    /// Internal counter for waveform generation
    counter: u16,
    /// Current output state (high/low)
    output: bool,
}

/// Noise channel
#[derive(Debug, Clone)]
pub struct NoiseChannel {
    /// Noise mode: false = periodic, true = white
    pub white_noise: bool,
    /// Shift rate (0-2 = fixed dividers, 3 = use tone 2 frequency)
    pub shift_rate: u8,
    /// 4-bit volume (0 = max, 15 = off)
    pub volume: u8,
    /// Linear feedback shift register (15-bit)
    lfsr: u16,
    /// Internal counter
    counter: u16,
}

impl Default for NoiseChannel {
    fn default() -> Self {
        Self {
            white_noise: false,
            shift_rate: 0,
            volume: 0x0F,  // Off
            lfsr: 0x8000,  // Initial seed
            counter: 0,
        }
    }
}

/// SN76489 PSG chip state
#[derive(Debug)]
pub struct Psg {
    /// Three tone channels
    pub tones: [ToneChannel; 3],
    /// Noise channel
    pub noise: NoiseChannel,
    /// Latched channel (0-3)
    latch_channel: u8,
    /// Latched type (false = frequency, true = volume)
    latch_volume: bool,
}

impl Default for Psg {
    fn default() -> Self {
        Self::new()
    }
}

impl Psg {
    /// Create a new PSG in reset state
    pub fn new() -> Self {
        let mut psg = Self {
            tones: Default::default(),
            noise: Default::default(),
            latch_channel: 0,
            latch_volume: false,
        };
        // Set all volumes to off (15)
        for tone in &mut psg.tones {
            tone.volume = 0x0F;
        }
        psg.noise.volume = 0x0F;
        psg
    }
    
    /// Reset the chip
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    
    /// Write a command byte to the PSG
    pub fn write(&mut self, value: u8) {
        if (value & 0x80) != 0 {
            // Latch byte: contains channel, type, and 4 bits of data
            self.latch_channel = (value >> 5) & 0x03;
            self.latch_volume = (value & 0x10) != 0;
            let data = value & 0x0F;
            
            if self.latch_volume {
                // Volume write
                self.write_volume(self.latch_channel, data);
            } else {
                // Frequency write (low 4 bits)
                self.write_frequency_low(self.latch_channel, data);
            }
        } else {
            // Data byte: 6 bits of additional frequency data
            if !self.latch_volume {
                self.write_frequency_high(self.latch_channel, value & 0x3F);
            }
        }
    }
    
    /// Write volume to a channel
    fn write_volume(&mut self, channel: u8, volume: u8) {
        match channel {
            0 => self.tones[0].volume = volume,
            1 => self.tones[1].volume = volume,
            2 => self.tones[2].volume = volume,
            3 => self.noise.volume = volume,
            _ => {}
        }
    }
    
    /// Write low 4 bits of frequency
    fn write_frequency_low(&mut self, channel: u8, data: u8) {
        match channel {
            0..=2 => {
                let tone = &mut self.tones[channel as usize];
                tone.frequency = (tone.frequency & 0x3F0) | (data as u16);
            }
            3 => {
                // Noise control register
                self.noise.white_noise = (data & 0x04) != 0;
                self.noise.shift_rate = data & 0x03;
                // Reset LFSR on noise mode change
                self.noise.lfsr = 0x8000;
            }
            _ => {}
        }
    }
    
    /// Write high 6 bits of frequency
    fn write_frequency_high(&mut self, channel: u8, data: u8) {
        if channel < 3 {
            let tone = &mut self.tones[channel as usize];
            tone.frequency = (tone.frequency & 0x00F) | ((data as u16) << 4);
        }
    }
    
    /// Step the PSG and generate a sample
    pub fn step(&mut self) -> i16 {
        let mut output: i32 = 0;
        
        // Process tone channels
        for tone in &mut self.tones {
            if tone.frequency > 0 {
                if tone.counter == 0 {
                    tone.counter = tone.frequency;
                    tone.output = !tone.output;
                } else {
                    tone.counter -= 1;
                }
            }
            
            if tone.output && tone.volume < 15 {
                // Volume table: 2dB per step, 0 = max, 15 = off
                let vol = VOLUME_TABLE[tone.volume as usize];
                output += vol as i32;
            }
        }
        
        // Process noise channel
        let noise_freq = match self.noise.shift_rate {
            0 => 0x10,   // N/512
            1 => 0x20,   // N/1024
            2 => 0x40,   // N/2048
            3 => self.tones[2].frequency,  // Tone 2 frequency
            _ => 0x10,
        };
        
        if noise_freq > 0 {
            if self.noise.counter == 0 {
                self.noise.counter = noise_freq;
                
                // Shift LFSR
                let feedback = if self.noise.white_noise {
                    // White noise: XOR bits 0 and 3
                    (self.noise.lfsr & 1) ^ ((self.noise.lfsr >> 3) & 1)
                } else {
                    // Periodic noise: just bit 0
                    self.noise.lfsr & 1
                };
                
                self.noise.lfsr = (self.noise.lfsr >> 1) | (feedback << 14);
            } else {
                self.noise.counter -= 1;
            }
        }
        
        if (self.noise.lfsr & 1) != 0 && self.noise.volume < 15 {
            let vol = VOLUME_TABLE[self.noise.volume as usize];
            output += vol as i32;
        }
        
        // Clamp to i16 range
        output.clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }
}

/// Volume lookup table (2dB per step, approximate)
const VOLUME_TABLE: [i16; 16] = [
    8191, 6507, 5168, 4105, 3261, 2590, 2057, 1634,
    1298, 1031, 819, 650, 516, 410, 326, 0,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_psg_new() {
        let psg = Psg::new();
        assert_eq!(psg.tones[0].frequency, 0);
        assert_eq!(psg.tones[0].volume, 15);
    }

    #[test]
    fn test_psg_reset() {
        let mut psg = Psg::new();
        psg.write(0x8A); // Set freq
        psg.write(0x90); // Set vol

        psg.reset();

        assert_eq!(psg.tones[0].frequency, 0);
        assert_eq!(psg.tones[0].volume, 15);
    }
    
    #[test]
    fn test_psg_volume_write() {
        let mut psg = Psg::new();
        
        // Write volume to channel 0: 1001 xxxx (90 = channel 0, volume, data 0)
        psg.write(0x90);  // Channel 0 volume = 0 (max)
        assert_eq!(psg.tones[0].volume, 0);
        
        // Write volume to channel 1: 1011 1111 (BF = channel 1, volume, data F)
        psg.write(0xBF);  // Channel 1 volume = 15 (off)
        assert_eq!(psg.tones[1].volume, 15);
    }
    
    #[test]
    fn test_psg_frequency_write() {
        let mut psg = Psg::new();
        
        // Write frequency to channel 0 (two-byte sequence)
        psg.write(0x85);  // Latch: channel 0, freq, low nibble = 5
        psg.write(0x3F);  // Data: high 6 bits = 0x3F
        
        // Frequency should be (0x3F << 4) | 0x05 = 0x3F5
        assert_eq!(psg.tones[0].frequency, 0x3F5);
    }
    
    #[test]
    fn test_psg_noise_mode() {
        let mut psg = Psg::new();
        
        // Write to noise channel: 1110 0111 (E7 = channel 3, freq, white noise, rate 3)
        psg.write(0xE7);
        
        assert!(psg.noise.white_noise);
        assert_eq!(psg.noise.shift_rate, 3);
    }

    #[test]
    fn test_psg_step_tone_generation() {
        let mut psg = Psg::new();

        // Set Channel 0 to Frequency 10, Volume 0 (Max)
        // Volume 0 => 8191

        // Frequency LSB (0x0A) | Latch (0x80) | Channel 0 (0x00) -> 0x8A
        psg.write(0x8A);
        psg.write(0x00); // Frequency MSB -> 0

        // Volume 0 (0x00) | Latch (0x80) | Channel 0 (0x00) | Type Vol (0x10) -> 0x90
        psg.write(0x90);

        assert_eq!(psg.tones[0].frequency, 10);
        assert_eq!(psg.tones[0].volume, 0);

        // Initial state: counter=0, output=false

        // Step 1: counter -> 10, output -> true. Result += 8191.
        let output = psg.step();
        assert_eq!(output, 8191);

        // Step 2..11: counter decrements 10->0. output stays true. Result += 8191.
        for _ in 0..10 {
            assert_eq!(psg.step(), 8191);
        }

        // Now counter should be 0.
        // Step 12: counter -> 10, output -> false. Result += 0.
        let output = psg.step();
        assert_eq!(output, 0);

        // Step 13..22: counter decrements. output false.
        for _ in 0..10 {
            assert_eq!(psg.step(), 0);
        }

        // Step 23: Toggle back to true
        assert_eq!(psg.step(), 8191);
    }

    #[test]
    fn test_psg_step_volume_mixing() {
        let mut psg = Psg::new();

        // Ch 0: Freq 10, Vol 0 (8191)
        psg.write(0x8A); psg.write(0x00); psg.write(0x90);

        // Ch 1: Freq 20, Vol 4 (3261)
        // Ch 1 Latch Freq: 0xA0 | 0x05 -> 0xA5 (Wait, 20 is 0x14. 4 low, 1 high)
        // Latch: 1010 0100 -> 0xA4
        // Data:  0000 0001 -> 0x01
        psg.write(0xA4); psg.write(0x01);

        // Ch 1 Vol 4: 1011 0100 -> 0xB4
        psg.write(0xB4);

        // Step 1: Both toggle to true (since counters start at 0)
        // Output = 8191 + 3261 = 11452
        assert_eq!(psg.step(), 11452);
    }

    #[test]
    fn test_psg_noise_generation() {
         let mut psg = Psg::new();

         // Enable noise channel
         // Periodic noise, rate 0 (0xE0)
         psg.write(0xE0);
         // Volume 0 (0xF0)
         psg.write(0xF0);

         // Run some steps and ensure output changes
         // Since it is periodic noise, it should eventually repeat or at least change

         let mut distinct_outputs = HashSet::new();
         // Step enough times. Rate 0 is N/512 (which means 0x10=16 in code?)
         // Code: 0 => 0x10. So period is 16+1=17 steps?
         // With initial seed 0x8000, it takes ~15 shifts for the bit to reach the end (bit 0).
         // 15 * 17 = 255 steps.
         // Let's run enough steps to cover a full cycle.
         for _ in 0..1000 {
             distinct_outputs.insert(psg.step());
         }

         // Should have at least two values (0 and 8191)
         assert!(distinct_outputs.len() >= 2);
         assert!(distinct_outputs.contains(&0));
         assert!(distinct_outputs.contains(&8191));
    }

    #[test]
    fn test_psg_noise_linked_to_tone2() {
        let mut psg = Psg::new();

        // Set Tone 2 Frequency to 4
        // Ch 2: 0x40 | 0x80 -> 0xC0. Latch C0 | 4 -> C4.
        psg.write(0xC4); psg.write(0x00);

        // Set Noise to use Tone 2 (Rate 3): 0xE3
        psg.write(0xE3);
        // Noise Vol 0: 0xF0
        psg.write(0xF0);

        // Initial LFSR check
        let initial_lfsr = psg.noise.lfsr;

        // Step 1: Counter 0 -> 4. LFSR updates.
        psg.step();
        assert_ne!(psg.noise.lfsr, initial_lfsr);

        let next_lfsr = psg.noise.lfsr;
        // Step 2..5: Counter decrements (4 steps). LFSR same.
        for _ in 0..4 {
            psg.step();
            assert_eq!(psg.noise.lfsr, next_lfsr);
        }

        // Step 6: Counter -> 0 -> 4. LFSR updates.
        psg.step();
        assert_ne!(psg.noise.lfsr, next_lfsr);
    }

    #[test]
    fn test_psg_volume_cutoff() {
        let mut psg = Psg::new();

        // Ch 0 Freq 10
        psg.write(0x8A); psg.write(0x00);

        // Vol 15 (Off)
        psg.write(0x9F);

        // Should be 0 even if toggled (Step 1 toggles output to true)
        assert_eq!(psg.step(), 0);

        // Change Volume to 14 (326)
        psg.write(0x9E);

        // Step 2: Output is still true (counter at 9). Should now output volume 14.
        assert_eq!(psg.step(), 326);
    }

    #[test]
    fn test_psg_frequency_zero() {
        let mut psg = Psg::new();

        // Set Freq 0 for Ch 0
        psg.write(0x80); psg.write(0x00);

        // Set Volume 0 (Max)
        psg.write(0x90);

        // Step. Counter is 0.
        // if frequency > 0 condition prevents update.
        // Output remains false.
        // Result should be 0.
        assert_eq!(psg.step(), 0);

        // Verify multiple steps
        for _ in 0..10 {
            assert_eq!(psg.step(), 0);
        }
    }
}
