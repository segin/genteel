//! Texas Instruments SN76489 Programmable Sound Generator (PSG)
//!
//! Refactored to use band-limited synthesis via BlipBuf for high quality.

use crate::apu::blip_buf::BlipBuf;
use crate::audio;
use serde::{Deserialize, Serialize};

fn default_master_clock() -> u32 {
    audio::NTSC_MCLK
}

fn default_sample_rate() -> u32 {
    audio::SAMPLE_RATE
}

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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoiseChannel {
    pub white_noise: bool,
    pub shift_rate: u8,
    pub volume: u8,
    pub lfsr: u16,
    pub counter: u16,
    pub last_amp: i32,
}

/// SN76489 PSG chip state
#[derive(Debug, Serialize, Deserialize)]
pub struct Psg {
    pub tones: [ToneChannel; 3],
    pub noise: NoiseChannel,
    pub latch_channel: u8,
    pub latch_volume: bool,
    #[serde(default = "default_master_clock")]
    pub master_clock: u32,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    /// Total MCLK cycles elapsed
    pub total_mclocks: u64,
    /// MCLK debt for PSG clock
    pub mclk_debt: u32,
    /// Band-limited synthesis buffer
    pub blip: BlipBuf,
}

impl Psg {
    fn volume_to_amp(volume: u8) -> i32 {
        if volume >= 0x0F {
            0
        } else {
            (4095.0 / 10f32.powf(volume as f32 / 10.0)).round() as i32
        }
    }

    pub fn new() -> Self {
        let mut psg = Self {
            tones: std::array::from_fn(|_| ToneChannel::default()),
            noise: NoiseChannel {
                volume: 0x0F,
                lfsr: 0x4000,
                ..Default::default()
            },
            latch_channel: 0,
            latch_volume: false,
            master_clock: default_master_clock(),
            sample_rate: default_sample_rate(),
            total_mclocks: 0,
            mclk_debt: 0,
            blip: BlipBuf::new(default_master_clock(), default_sample_rate()),
        };
        for tone in &mut psg.tones {
            tone.volume = 0x0F;
        }
        psg
    }

    pub fn reset(&mut self) {
        let master_clock = self.master_clock;
        let sample_rate = self.sample_rate;
        *self = Self::new();
        self.set_timing(master_clock, sample_rate);
    }

    /// Reconfigure PSG timing for the active video region and output sample rate.
    pub fn set_timing(&mut self, master_clock: u32, sample_rate: u32) {
        if self.master_clock == master_clock
            && self.sample_rate == sample_rate
            && self.blip.clock_rate() == master_clock
            && self.blip.sample_rate() == sample_rate
        {
            return;
        }
        self.master_clock = master_clock;
        self.sample_rate = sample_rate;
        self.blip.set_timing(master_clock, sample_rate);
    }

    pub fn write(&mut self, value: u8) {
        if (value & 0x80) != 0 {
            // Latch/Control
            self.latch_channel = (value >> 5) & 0x03;
            self.latch_volume = (value & 0x10) != 0;
            let data = value & 0x0F;
            if self.latch_volume {
                self.write_volume(self.latch_channel, data);
            } else {
                self.write_frequency_low(self.latch_channel, data);
            }
        } else {
            // Data
            let data = value & 0x3F;
            if self.latch_volume {
                self.write_volume(self.latch_channel, data & 0x0F);
            } else {
                self.write_frequency_high(self.latch_channel, data);
            }
        }
    }

    pub fn update_channel_amp(&mut self, channel: u8) {
        let clock = self.total_mclocks;
        match channel {
            0..=2 => {
                let (output, volume) = {
                    let t = &self.tones[channel as usize];
                    (t.output, t.volume)
                };
                let new_amp = if output {
                    Self::volume_to_amp(volume)
                } else {
                    0
                };
                let delta = new_amp - self.tones[channel as usize].last_amp;
                self.blip.add_delta(clock, delta);
                self.tones[channel as usize].last_amp = new_amp;
            }
            3 => {
                let (output, volume) = { (self.noise.lfsr & 1 != 0, self.noise.volume) };
                let new_amp = if output {
                    Self::volume_to_amp(volume)
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
        if channel < 3 {
            self.tones[channel as usize].volume = volume;
            self.update_channel_amp(channel);
        } else {
            self.noise.volume = volume;
            self.update_channel_amp(3);
        }
    }

    fn write_frequency_low(&mut self, channel: u8, data: u8) {
        if channel < 3 {
            self.tones[channel as usize].frequency =
                (self.tones[channel as usize].frequency & 0x3F0) | (data as u16);
        } else {
            self.noise.white_noise = (data & 0x04) != 0;
            self.noise.shift_rate = data & 0x03;
            self.noise.lfsr = 0x4000;
            self.update_channel_amp(3);
        }
    }

    fn write_frequency_high(&mut self, channel: u8, data: u8) {
        if channel < 3 {
            self.tones[channel as usize].frequency =
                (self.tones[channel as usize].frequency & 0x00F) | ((data as u16) << 4);
        }
    }

    fn step_psg_clock(&mut self) {
        self.total_mclocks += 15;

        let noise_freq = match self.noise.shift_rate {
            0 => 0x10,
            1 => 0x20,
            2 => 0x40,
            3 => self.tones[2].frequency,
            _ => 0x10,
        };

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
    }

    /// Step the PSG by a number of PSG clock ticks.
    pub fn step_cycles(&mut self, cycles: u32) {
        for _ in 0..cycles {
            self.step_psg_clock();
        }
    }

    /// Step the PSG using M68K cycles from the system bus.
    pub fn step_m68k_cycles(&mut self, cycles: u32) {
        self.mclk_debt += cycles * 7;
        while self.mclk_debt >= 15 {
            self.mclk_debt -= 15;
            self.step_psg_clock();
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
        for (i, sample) in s.iter_mut().enumerate().take(3) {
            *sample = self.tones[i].last_amp as i16;
        }
        s[3] = self.noise.last_amp as i16;
        s
    }

    /// Step the PSG and generate a sample (legacy, now 1 cycle)
    pub fn step(&mut self) -> i16 {
        self.step_cycles(1);
        let mut buf = [0i16; 1];
        if self.blip.read_samples(&mut buf[..]) > 0 {
            buf[0]
        } else {
            self.blip.read_instant()
        }
    }

    pub fn generate_sample(&mut self) -> i16 {
        // Band-limited output through the BlipBuf sinc kernel (see
        // Ym2612::generate_sample).
        let mut buf = [0i16; 1];
        self.blip.read_samples(&mut buf[..]);
        buf[0]
    }
}

impl Default for Psg {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio;

    #[test]
    fn test_psg_initialization() {
        let psg = Psg::new();
        for tone in &psg.tones {
            assert_eq!(tone.volume, 0x0F);
            assert_eq!(tone.frequency, 0);
        }
        assert_eq!(psg.noise.volume, 0x0F);
        assert_eq!(psg.noise.lfsr, 0x4000);
        assert_eq!(psg.latch_channel, 0);
        assert_eq!(psg.latch_volume, false);
    }

    #[test]
    fn test_psg_reset() {
        let mut psg = Psg::new();
        psg.write(0x8A); // Change some state (latch channel 0, freq low)
        psg.write(0x01); // freq high
        psg.write(0x90); // Vol channel 0

        psg.reset();

        for tone in &psg.tones {
            assert_eq!(tone.volume, 0x0F);
            assert_eq!(tone.frequency, 0);
        }
        assert_eq!(psg.latch_channel, 0);
    }

    #[test]
    fn test_psg_write_latch_frequency() {
        let mut psg = Psg::new();
        // Channel 0, Frequency write
        // 1000_1010 => Latch=1, Ch=0, Type=Freq, Data=1010
        psg.write(0x8A);
        assert_eq!(psg.latch_channel, 0);
        assert_eq!(psg.latch_volume, false);
        assert_eq!(psg.tones[0].frequency, 0xA);

        // Second byte: Data write
        // 0000_0001 => Latch=0, Data=0001
        psg.write(0x01);
        assert_eq!(psg.tones[0].frequency, 0x1A); // (1 << 4) | 0xA
    }

    #[test]
    fn test_psg_write_latch_volume() {
        let mut psg = Psg::new();
        // Channel 1, Volume write
        // 1011_0100 => Latch=1, Ch=1, Type=Vol, Data=0100
        psg.write(0xB4);
        assert_eq!(psg.latch_channel, 1);
        assert_eq!(psg.latch_volume, true);
        assert_eq!(psg.tones[1].volume, 4);

        // Data write to volume latch
        // 0000_0011 => Latch=0, Data=0011
        psg.write(0x03);
        assert_eq!(psg.tones[1].volume, 3);
    }

    #[test]
    fn test_psg_step_m68k_cycles() {
        let mut psg = Psg::new();
        psg.step_m68k_cycles(1);
        assert_eq!(psg.mclk_debt, 7); // 1 * 7

        psg.step_m68k_cycles(1);
        assert_eq!(psg.mclk_debt, 14); // 7 + 7 = 14

        psg.step_m68k_cycles(1);
        // debt was 14, adds 7 (total 21). While loop subtracts 15 => 6
        assert_eq!(psg.mclk_debt, 6);
    }

    #[test]
    fn test_psg_get_channel_samples() {
        let mut psg = Psg::new();
        psg.tones[0].last_amp = 100;
        psg.tones[1].last_amp = 200;
        psg.tones[2].last_amp = 300;
        psg.noise.last_amp = 400;

        let samples = psg.get_channel_samples();
        assert_eq!(samples, [100, 200, 300, 400]);
    }

    #[test]
    fn test_psg_set_timing_updates_blip_rates() {
        let mut psg = Psg::new();

        psg.set_timing(audio::PAL_MCLK, 48_000);

        assert_eq!(psg.master_clock, audio::PAL_MCLK);
        assert_eq!(psg.sample_rate, 48_000);
        assert_eq!(psg.blip.clock_rate(), audio::PAL_MCLK);
        assert_eq!(psg.blip.sample_rate(), 48_000);
    }

    #[test]
    fn test_psg_set_timing_same_values_is_noop() {
        let mut psg = Psg::new();
        psg.tones[0].output = true;
        psg.tones[0].volume = 0;
        psg.tones[0].last_amp = 0;
        psg.update_channel_amp(0);
        assert!(psg.blip.read_instant() > 0);

        psg.set_timing(audio::NTSC_MCLK, audio::SAMPLE_RATE);

        assert_eq!(psg.master_clock, audio::NTSC_MCLK);
        assert_eq!(psg.sample_rate, audio::SAMPLE_RATE);
        assert!(psg.blip.read_instant() > 0);
    }
}
