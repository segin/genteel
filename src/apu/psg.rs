//! SN76489 PSG (Programmable Sound Generator)
//!
//! Refactored to use BlipBuf for band-limited synthesis.

use crate::apu::blip_buf::BlipBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsgTone {
    pub frequency: u16,
    pub volume: u8,
    pub counter: u16,
    pub output: bool,
    pub last_amp: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsgNoise {
    pub volume: u8,
    pub counter: u16,
    pub output: bool,
    pub shift_register: u16,
    pub white: bool,
    pub rate: u8,
    pub last_amp: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Psg {
    pub tones: [PsgTone; 3],
    pub noise: PsgNoise,
    pub latched_channel: u8,
    pub latched_type: bool, // false=freq, true=vol
    pub total_clocks: u64,
    pub blip: BlipBuf,
    last_mixed: i32,
    pub clock_accumulator: f32,
}

impl Psg {
    pub fn new() -> Self {
        Self {
            tones: std::array::from_fn(|_| PsgTone {
                frequency: 0,
                volume: 0x0F,
                counter: 0,
                output: false,
                last_amp: 0,
            }),
            noise: PsgNoise {
                volume: 0x0F,
                counter: 0,
                output: false,
                shift_register: 0x8000,
                white: false,
                rate: 0,
                last_amp: 0,
            },
            latched_channel: 0,
            latched_type: false,
            total_clocks: 0,
            blip: BlipBuf::new(3579545 / 16, 53267),
            last_mixed: 0,
            clock_accumulator: 0.0,
        }
    }

    pub fn reset(&mut self) {
        let blip = self.blip.clone();
        *self = Self::new();
        self.blip = blip;
        self.blip.clear();
    }

    pub fn write(&mut self, data: u8) {
        if (data & 0x80) != 0 {
            // Latch byte
            self.latched_channel = (data >> 5) & 0x03;
            self.latched_type = (data & 0x10) != 0;
            let val = data & 0x0F;
            if self.latched_type {
                // Volume
                if self.latched_channel < 3 {
                    self.tones[self.latched_channel as usize].volume = val as u8;
                    self.update_tone_amp(self.latched_channel as usize);
                } else {
                    self.noise.volume = val as u8;
                    self.update_noise_amp();
                }
            } else {
                // Frequency / Noise Mode
                if self.latched_channel < 3 {
                    let ch = self.latched_channel as usize;
                    self.tones[ch].frequency = (self.tones[ch].frequency & 0x3F0) | val as u16;
                } else {
                    self.noise.white = (val & 0x04) != 0;
                    self.noise.rate = (val & 0x03) as u8;
                    self.noise.shift_register = 0x8000;
                }
            }
        } else {
            // Data byte
            let val = data & 0x3F;
            if self.latched_channel < 3 {
                let ch = self.latched_channel as usize;
                if !self.latched_type {
                    self.tones[ch].frequency = (self.tones[ch].frequency & 0x00F) | ((val as u16) << 4);
                } else {
                    self.tones[ch].volume = (val & 0x0F) as u8;
                    self.update_tone_amp(ch);
                }
            } else {
                if !self.latched_type {
                    self.noise.white = (val & 0x04) != 0;
                    self.noise.rate = (val & 0x03) as u8;
                    self.noise.shift_register = 0x8000;
                } else {
                    self.noise.volume = (val & 0x0F) as u8;
                    self.update_noise_amp();
                }
            }
        }
    }

    pub fn step_cycles(&mut self, m68k_cycles: u32) {
        // PSG clock is Master Clock / 32. Or Z80 clock / 16.
        // m68k_cycles is 7 master clocks.
        // PSG clocks = (m68k_cycles * 7) / 32
        self.clock_accumulator += (m68k_cycles as f32 * 7.0) / 32.0;
        
        while self.clock_accumulator >= 1.0 {
            self.total_clocks += 1;
            self.clock_accumulator -= 1.0;
            
            let mut changed = false;

            // Tone Channels
            for i in 0..3 {
                if self.tones[i].counter > 0 {
                    self.tones[i].counter -= 1;
                }
                if self.tones[i].counter == 0 {
                    self.tones[i].output = !self.tones[i].output;
                    self.tones[i].counter = self.tones[i].frequency;
                    self.update_tone_amp(i);
                    changed = true;
                }
            }

            // Noise Channel
            if self.noise.counter > 0 {
                self.noise.counter -= 1;
            }
            if self.noise.counter == 0 {
                self.noise.output = !self.noise.output;
                if self.noise.output {
                    // Clock shift register on rising edge
                    let feedback = if self.noise.white {
                        let b = self.noise.shift_register;
                        ((b >> 0) ^ (b >> 3)) & 1
                    } else {
                        self.noise.shift_register & 1
                    };
                    self.noise.shift_register = (self.noise.shift_register >> 1) | (feedback << 14);
                    self.update_noise_amp();
                    changed = true;
                }
                
                self.noise.counter = match self.noise.rate {
                    0 => 0x10,
                    1 => 0x20,
                    2 => 0x40,
                    3 => self.tones[2].frequency,
                    _ => 0x10,
                };
            }

            if changed {
                let mixed = self.get_mixed_amp();
                let delta = mixed - self.last_mixed;
                if delta != 0 {
                    self.blip.add_delta(self.total_clocks, delta);
                    self.last_mixed = mixed;
                }
            }
        }
    }

    fn update_tone_amp(&mut self, i: usize) {
        if self.tones[i].output && self.tones[i].frequency > 1 {
            self.tones[i].last_amp = self.attenuate(self.tones[i].volume);
        } else {
            self.tones[i].last_amp = 0;
        }
    }

    fn update_noise_amp(&mut self) {
        if (self.noise.shift_register & 1) != 0 {
            self.noise.last_amp = self.attenuate(self.noise.volume);
        } else {
            self.noise.last_amp = 0;
        }
    }

    fn attenuate(&self, volume: u8) -> i16 {
        if volume >= 15 { return 0; }
        (4095.0 * f64::powf(10.0, (volume as f64 * -2.0) / 20.0)) as i16
    }

    fn get_mixed_amp(&self) -> i32 {
        let mut sum = 0i32;
        for i in 0..3 { sum += self.tones[i].last_amp as i32; }
        sum += self.noise.last_amp as i32;
        sum
    }

    pub fn current_sample(&mut self) -> i16 {
        let mut buf = [0i16; 1];
        if self.blip.read_samples(&mut buf) > 0 {
            self.total_clocks = 0;
            buf[0]
        } else {
            self.blip.read_instant()
        }
    }

    pub fn get_channel_samples(&self) -> [i16; 4] {
        let mut res = [0i16; 4];
        for i in 0..3 { res[i] = self.tones[i].last_amp; }
        res[3] = self.noise.last_amp;
        res
    }
}
