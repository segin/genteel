//! Texas Instruments SN76489 Programmable Sound Generator (PSG)
//!
//! Refactored to use band-limited synthesis via BlipBuf for high quality.

use crate::apu::blip_buf::BlipBuf;
use serde::{Deserialize, Serialize};

/// Square wave tone channel
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToneChannel {
    /// 10-bit frequency divider
    pub frequency: u16,
    /// 4-bit volume (0 = max, 15 = off)
    pub volume: u8,
    /// Internal counter
    pub counter: u16,
    /// Current output state
    pub output: bool,
    /// Last output amplitude added to blip_buf
    pub last_amp: i32,
}

/// Noise channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseChannel {
    pub white_noise: bool,
    pub shift_rate: u8,
    pub volume: u8,
    pub lfsr: u16,
    pub counter: u16,
    pub last_amp: i32,
}

impl Default for NoiseChannel {
    fn default() -> Self {
        Self {
            white_noise: false,
            shift_rate: 0,
            volume: 0x0F,
            lfsr: 0x4000, // 15-bit LFSR seed
            counter: 0,
            last_amp: 0,
        }
    }
}

/// SN76489 PSG chip state
#[derive(Debug, Serialize, Deserialize)]
pub struct Psg {
    pub tones: [ToneChannel; 3],
    pub noise: NoiseChannel,
    latch_channel: u8,
    latch_volume: bool,
    /// Total cycles elapsed
    pub total_clocks: u64,
    /// Band-limited synthesis buffer
    pub blip: BlipBuf,
}

impl Psg {
    pub fn new() -> Self {
        let mut psg = Self {
            tones: Default::default(),
            noise: Default::default(),
            latch_channel: 0,
            latch_volume: false,
            total_clocks: 1, // Start at 1 to allow delta at 0 if needed
            blip: BlipBuf::new(3579545, 53267), // Clocked at ~3.58MHz, output at FM rate
        };
        for tone in &mut psg.tones {
            tone.volume = 0x0F;
            tone.last_amp = 0;
        }
        psg.noise.volume = 0x0F;
        psg.noise.last_amp = 0;
        psg
    }

    pub fn reset(&mut self) {
        let blip = self.blip.clone();
        *self = Self::new();
        self.blip = blip;
        self.blip.clear();
    }

    pub fn write(&mut self, value: u8) {
        if (value & 0x80) != 0 {
            self.latch_channel = (value >> 5) & 0x03;
            self.latch_volume = (value & 0x10) != 0;
            let data = value & 0x0F;
            if self.latch_volume {
                self.write_volume(self.latch_channel, data);
            } else {
                self.write_frequency_low(self.latch_channel, data);
            }
        } else if !self.latch_volume {
            self.write_frequency_high(self.latch_channel, value & 0x3F);
        }
    }

    pub fn update_channel_amp(&mut self, channel: u8) {
        let clock = self.total_clocks;
        match channel {
            0..=2 => {
                let (output, volume) = {
                    let tone = &self.tones[channel as usize];
                    (tone.output, tone.volume)
                };
                let new_amp = if output && volume < 15 {
                    VOLUME_TABLE[volume as usize] as i32
                } else {
                    0
                };
                let delta = new_amp - self.tones[channel as usize].last_amp;
                self.blip.add_delta(clock, delta);
                self.tones[channel as usize].last_amp = new_amp;
            }
            3 => {
                let new_amp = if (self.noise.lfsr & 1) != 0 && self.noise.volume < 15 {
                    VOLUME_TABLE[self.noise.volume as usize] as i32
                } else {
                    0
                };
                let delta = new_amp - self.noise.last_amp;
                self.blip.add_delta(clock, delta);
                self.noise.last_amp = new_amp;
            }
            _ => {}
        }
    }

    fn write_volume(&mut self, channel: u8, volume: u8) {
        match channel {
            0..=2 => self.tones[channel as usize].volume = volume,
            3 => self.noise.volume = volume,
            _ => {}
        }
        self.update_channel_amp(channel);
    }

    fn write_frequency_low(&mut self, channel: u8, data: u8) {
        match channel {
            0..=2 => {
                self.tones[channel as usize].frequency =
                    (self.tones[channel as usize].frequency & 0x3F0) | (data as u16);
            }
            3 => {
                self.noise.white_noise = (data & 0x04) != 0;
                self.noise.shift_rate = data & 0x03;
                self.noise.lfsr = 0x4000;
                self.update_channel_amp(3);
            }
            _ => {}
        }
    }

    fn write_frequency_high(&mut self, channel: u8, data: u8) {
        if channel < 3 {
            self.tones[channel as usize].frequency =
                (self.tones[channel as usize].frequency & 0x00F) | ((data as u16) << 4);
        }
    }

    /// Step the PSG and populate BlipBuf with deltas
    pub fn step_cycles(&mut self, cycles: u32) {
        let noise_freq = match self.noise.shift_rate {
            0 => 0x10,
            1 => 0x20,
            2 => 0x40,
            3 => self.tones[2].frequency,
            _ => 0x10,
        };

        for _ in 0..cycles {
            // 1. Update Tones
            for i in 0..3 {
                let freq = if self.tones[i].frequency == 0 {
                    0x400
                } else {
                    self.tones[i].frequency
                };
                if self.tones[i].counter > 0 {
                    self.tones[i].counter -= 1;
                }
                if self.tones[i].counter == 0 {
                    self.tones[i].output = !self.tones[i].output;
                    self.tones[i].counter = freq;
                    self.update_channel_amp(i as u8);
                }
            }

            // 2. Update Noise
            let n_freq = if noise_freq == 0 { 0x400 } else { noise_freq };
            if self.noise.counter > 0 {
                self.noise.counter -= 1;
            }
            if self.noise.counter == 0 {
                self.noise.counter = n_freq;

                let feedback = if self.noise.white_noise {
                    ((self.noise.lfsr & 1) ^ ((self.noise.lfsr >> 1) & 1)) & 1
                } else {
                    self.noise.lfsr & 1
                };
                self.noise.lfsr = (self.noise.lfsr >> 1) | (feedback << 14);
                self.update_channel_amp(3);
            }
            self.total_clocks += 1;
        }
    }

    pub fn current_sample(&self) -> i16 {
        let mut out = 0i32;
        for i in 0..3 {
            out += self.tones[i].last_amp;
        }
        out += self.noise.last_amp;
        out.clamp(-32768, 32767) as i16
    }

    pub fn get_channel_samples(&self) -> [i16; 4] {
        let mut s = [0i16; 4];
        for (i, item) in s.iter_mut().enumerate().take(3) {
            *item = self.tones[i].last_amp as i16;
        }
        s[3] = self.noise.last_amp as i16;
        s
    }

    /// Step the PSG and generate a sample (legacy, now 1 cycle)
    pub fn step(&mut self) -> i16 {
        self.step_cycles(1);
        let mut buf = [0i16; 1];
        if self.blip.read_samples(&mut buf) > 0 {
            buf[0]
        } else {
            self.blip.read_instant()
        }
    }

    pub fn generate_sample(&mut self) -> i16 {
        let mut buf = [0i16; 1];
        if self.blip.read_samples(&mut buf) > 0 {
            buf[0]
        } else {
            self.blip.read_instant()
        }
    }
}

const VOLUME_TABLE: [i16; 16] = [
    4095, 3253, 2584, 2052, 1630, 1295, 1028, 817, 649, 515, 409, 325, 258, 205, 163, 0,
];

impl Default for Psg {
    fn default() -> Self {
        Self::new()
    }
}
