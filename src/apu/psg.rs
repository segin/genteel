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

use serde::{Deserialize, Serialize};

/// Square wave tone channel
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToneChannel {
    /// 10-bit frequency divider (higher = lower pitch)
    pub frequency: u16,
    /// 4-bit volume (0 = max, 15 = off)
    pub volume: u8,
    /// Internal counter for waveform generation
    pub counter: u16,
    /// Current output state (high/low)
    pub output: bool,
}

/// Noise channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseChannel {
    /// Noise mode: false = periodic, true = white
    pub white_noise: bool,
    /// Shift rate (0-2 = fixed dividers, 3 = use tone 2 frequency)
    pub shift_rate: u8,
    /// 4-bit volume (0 = max, 15 = off)
    pub volume: u8,
    /// Linear feedback shift register (15-bit)
    pub lfsr: u16,
    /// Internal counter
    pub counter: u16,
}

impl Default for NoiseChannel {
    fn default() -> Self {
        Self {
            white_noise: false,
            shift_rate: 0,
            volume: 0x0F, // Off
            lfsr: 0x8000, // Initial seed
            counter: 0,
        }
    }
}

/// SN76489 PSG chip state
#[derive(Debug, Serialize, Deserialize)]
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

    /// Step the PSG by a number of clock cycles and return an averaged sample.
    /// The PSG is clocked at ~3.58 MHz on the Genesis.
    pub fn step_cycles(&mut self, cycles: u32) -> i16 {
        if cycles == 0 {
            return self.current_sample();
        }

        let mut total_output: i32 = 0;

        for _ in 0..cycles {
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
            }

            // Process noise channel
            let noise_freq = match self.noise.shift_rate {
                0 => 0x10,                    // N/512
                1 => 0x20,                    // N/1024
                2 => 0x40,                    // N/2048
                3 => self.tones[2].frequency, // Tone 2 frequency
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

            total_output += self.current_sample() as i32;
        }

        (total_output / cycles as i32) as i16
    }

    /// Generate a single instantaneous sample from current state
    fn current_sample(&self) -> i16 {
        let mut output: i32 = 0;

        for tone in &self.tones {
            if tone.output && tone.volume < 15 {
                output += VOLUME_TABLE[tone.volume as usize] as i32;
            }
        }

        if (self.noise.lfsr & 1) != 0 && self.noise.volume < 15 {
            output += VOLUME_TABLE[self.noise.volume as usize] as i32;
        }

        output.clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }

    /// Step the PSG and generate a sample (legacy, now 1 cycle)
    pub fn step(&mut self) -> i16 {
        self.step_cycles(1)
    }
}

/// Volume lookup table (2dB per step, approximate)
const VOLUME_TABLE: [i16; 16] = [
    8191, 6507, 5168, 4105, 3261, 2590, 2057, 1634, 1298, 1031, 819, 650, 516, 410, 326, 0,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psg_new() {
        let psg = Psg::new();
        assert_eq!(psg.tones[0].frequency, 0);
        assert_eq!(psg.tones[0].volume, 15);
    }

    #[test]
    fn test_psg_volume_write() {
        let mut psg = Psg::new();

        // Write volume to channel 0: 1001 xxxx (90 = channel 0, volume, data 0)
        psg.write(0x90); // Channel 0 volume = 0 (max)
        assert_eq!(psg.tones[0].volume, 0);

        // Write volume to channel 1: 1011 1111 (BF = channel 1, volume, data F)
        psg.write(0xBF); // Channel 1 volume = 15 (off)
        assert_eq!(psg.tones[1].volume, 15);
    }

    #[test]
    fn test_psg_frequency_write() {
        let mut psg = Psg::new();

        // Write frequency to channel 0 (two-byte sequence)
        psg.write(0x85); // Latch: channel 0, freq, low nibble = 5
        psg.write(0x3F); // Data: high 6 bits = 0x3F

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

        // Channel 0: freq = 4, volume = 0 (max)
        psg.tones[0].frequency = 4;
        psg.tones[0].volume = 0;
        psg.tones[0].counter = 0; // Ensure starts at 0
        psg.tones[0].output = false; // Ensure starts low

        // Counter = 0 -> reset to 4, flip output (true)
        // Step 1: output becomes high (true)
        let sample = psg.step();
        assert_eq!(sample, VOLUME_TABLE[0]);

        // Counter was reset to 4.
        // Steps 2, 3, 4, 5: counter decrements 4->3, 3->2, 2->1, 1->0
        // Output remains high.
        for _ in 0..4 {
            let s = psg.step();
            assert_eq!(s, VOLUME_TABLE[0]);
        }

        // Step 6: counter is 0. Reset to 4, flip output (false).
        // Output becomes low.
        let sample = psg.step();
        assert_eq!(sample, 0); // When output is low (false), tone logic doesn't add volume.

        // Steps 7, 8, 9, 10: counter decrements 4->3, 3->2, 2->1, 1->0
        for _ in 0..4 {
            let s = psg.step();
            assert_eq!(s, 0);
        }

        // Step 11: counter is 0. Reset to 4, flip output (true).
        let sample = psg.step();
        assert_eq!(sample, VOLUME_TABLE[0]);
    }

    #[test]
    fn test_psg_step_mixing() {
        let mut psg = Psg::new();

        // Channel 0: freq 10, vol 0 (max ~8191)
        psg.tones[0].frequency = 10;
        psg.tones[0].volume = 0;
        psg.tones[0].output = true; // Force high
        psg.tones[0].counter = 5;

        // Channel 1: freq 20, vol 4 (~3261)
        psg.tones[1].frequency = 20;
        psg.tones[1].volume = 4;
        psg.tones[1].output = true; // Force high
        psg.tones[1].counter = 5;

        // Mute others
        psg.tones[2].volume = 15;
        psg.noise.volume = 15;

        let sample = psg.step();
        let expected = VOLUME_TABLE[0] as i32 + VOLUME_TABLE[4] as i32;
        assert_eq!(sample as i32, expected);
    }

    #[test]
    fn test_psg_step_volume_cutoff() {
        let mut psg = Psg::new();
        psg.tones[0].frequency = 10;
        psg.tones[0].output = true;
        psg.tones[0].counter = 5; // Non-zero so it doesn't flip immediately

        // Volume 15 = off
        psg.tones[0].volume = 15;
        // Step decrements counter to 4, output stays true.
        assert_eq!(psg.step(), 0);

        // Volume 0 = max
        psg.tones[0].volume = 0;
        // Step decrements counter to 3, output stays true.
        assert_eq!(psg.step(), VOLUME_TABLE[0]);
    }

    #[test]
    fn test_psg_step_noise_generation() {
        let mut psg = Psg::new();
        // Setup noise: White noise, Rate 0 (N/512 => 0x10 = 16)
        psg.noise.white_noise = true;
        psg.noise.shift_rate = 0;
        psg.noise.volume = 0; // Max volume
        psg.noise.lfsr = 0x8000; // Seed
        psg.noise.counter = 0;

        // Step 1: counter 0 -> reload 16. Shift LFSR.
        // LFSR 0x8000 (1000...0000). Bit 0 is 0.
        // White noise feedback: (bit0 ^ bit3). 0^0 = 0.
        // New LFSR = (0x8000 >> 1) | (0 << 14) = 0x4000.
        // Output checks (lfsr & 1). 0x4000 & 1 = 0. Output 0.
        let s1 = psg.step();
        assert_eq!(s1, 0);

        // We need to advance enough to get a 1 in bit 0.
        // The LFSR shifts right. The feedback goes into bit 14.
        // Eventually a 1 will reach bit 0.

        // Let's just run for a while and verify we get non-zero output at some point.
        let mut saw_high = false;
        let mut saw_low = false;

        // Run enough cycles.
        for _ in 0..1000 {
            let s = psg.step();
            if s > 0 {
                saw_high = true;
            }
            if s == 0 {
                saw_low = true;
            }
        }

        assert!(saw_high, "Noise should produce high output");
        assert!(saw_low, "Noise should produce low output");
    }

    #[test]
    fn test_psg_tone_step_v2() {
        let mut psg = Psg::new();

        // Setup Tone 0: Frequency = 2 (write 2), Volume = 0
        // Write: 1000 0010 (82) -> Channel 0, Freq, Data 2
        psg.write(0x82);
        // Write: 1001 0000 (90) -> Channel 0, Vol, Data 0
        psg.write(0x90);

        assert_eq!(psg.tones[0].frequency, 2);
        assert_eq!(psg.tones[0].volume, 0);

        // Initial state: counter = 0, output = false (default)
        // Step 1: counter is 0, so it resets to frequency (2) and toggles output (true)
        let sample = psg.step();
        assert_eq!(psg.tones[0].counter, 2);
        assert!(psg.tones[0].output);
        assert_eq!(sample, VOLUME_TABLE[0]); // Max volume

        // Step 2: counter = 2 -> 1
        let sample = psg.step();
        assert_eq!(psg.tones[0].counter, 1);
        assert!(psg.tones[0].output); // Still high
        assert_eq!(sample, VOLUME_TABLE[0]);

        // Step 3: counter = 1 -> 0
        let sample = psg.step();
        assert_eq!(psg.tones[0].counter, 0);
        assert!(psg.tones[0].output); // Still high
        assert_eq!(sample, VOLUME_TABLE[0]);

        // Step 4: counter = 0 -> resets to 2, toggles output (false)
        let sample = psg.step();
        assert_eq!(psg.tones[0].counter, 2);
        assert!(!psg.tones[0].output);
        assert_eq!(sample, 0); // Output low -> 0 contribution

        // Step 5: counter = 2 -> 1
        let sample = psg.step();
        assert_eq!(psg.tones[0].counter, 1);
        assert!(!psg.tones[0].output);
        assert_eq!(sample, 0);

        // Step 6: counter = 1 -> 0
        let sample = psg.step();
        assert_eq!(psg.tones[0].counter, 0);
        assert!(!psg.tones[0].output);
        assert_eq!(sample, 0);

        // Step 7: counter = 0 -> resets to 2, toggles output (true)
        let sample = psg.step();
        assert_eq!(psg.tones[0].counter, 2);
        assert!(psg.tones[0].output);
        assert_eq!(sample, VOLUME_TABLE[0]);
    }

    #[test]
    fn test_psg_volume_mixing_v2() {
        let mut psg = Psg::new();

        // Channel 0: Freq 1, Vol 0 (Max)
        psg.write(0x81);
        psg.write(0x90);

        // Channel 1: Freq 1, Vol 4
        psg.write(0xA1); // Channel 1 (10), Freq (0), Data 1 -> 1010 0001
        psg.write(0xB4); // Channel 1 (10), Vol (1), Data 4 -> 1011 0100

        // Step 1: Both toggle to High
        let sample = psg.step();
        let expected = (VOLUME_TABLE[0] as i32 + VOLUME_TABLE[4] as i32) as i16;
        assert_eq!(sample, expected);
    }

    #[test]
    fn test_psg_noise_generation_v2() {
        let mut psg = Psg::new();

        // Setup Noise: White Noise, Rate 0 (N/512 -> 0x10 = 16)
        // 1110 0100 (E4) -> Ch 3, Type 0, Data 4 (White=1, Rate=00)
        psg.write(0xE4);

        // Set Volume to Max (0)
        // 1111 0000 (F0)
        psg.write(0xF0);

        assert!(psg.noise.white_noise);
        assert_eq!(psg.noise.shift_rate, 0);

        // Initial LFSR is 0x8000. Bit 0 is 0.
        // Step 1: counter = 0 -> reset to 16. Shift LFSR.
        // Feedback (White): (bit0 ^ bit3).
        // LFSR 0x8000: bit0=0, bit3=0. Feedback=0.
        // New LFSR = (0x8000 >> 1) | (0 << 14) = 0x4000.
        // Output: bit0 of new LFSR?
        // Code checks `if (self.noise.lfsr & 1) != 0`.
        // 0x4000 & 1 = 0. Output 0.

        let sample = psg.step();
        assert_eq!(psg.noise.counter, 16);
        assert_eq!(psg.noise.lfsr, 0x4000);
        assert_eq!(sample, 0);

        // We need to step enough times to get a 1 in bit 0.
        // Run until we see output.
        let mut seen_noise = false;
        // 16 cycles per shift * 16 shifts = 256 steps roughly.
        for _ in 0..1000 {
            if psg.step() > 0 {
                seen_noise = true;
                break;
            }
        }
        assert!(seen_noise, "Noise channel should eventually produce output");
    }

    #[test]
    fn test_psg_edge_cases() {
        let mut psg = Psg::new();

        // 1. Volume 15 (Silence)
        psg.write(0x81); // Tone 0 Freq 1
        psg.write(0x9F); // Tone 0 Vol 15

        let sample = psg.step();
        assert_eq!(sample, 0);

        // 2. Frequency 0 (should be treated as 0 or handled gracefully)
        // Code: `if tone.frequency > 0`. So if 0, it doesn't count down.
        let mut psg = Psg::new();
        psg.write(0x80); // Tone 0 Freq 0 (1000 0000)
        psg.write(0x90); // Tone 0 Vol 0

        // Manually set freq to 0 just to be sure (write might limit it?)
        // Write low: 0x80 -> freq = ... | 0.
        // Write high: default 0.
        assert_eq!(psg.tones[0].frequency, 0);

        let sample = psg.step();
        // Counter should not change (starts at 0)
        assert_eq!(psg.tones[0].counter, 0);
        // Output shouldn't toggle (starts false)
        assert!(!psg.tones[0].output);
        // Result 0
        assert_eq!(sample, 0);
    }
}
