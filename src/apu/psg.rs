//! SN76489-compatible Programmable Sound Generator (PSG) as integrated in
//! the Sega VDP (315-5313).
//!
//! The PSG is clocked at MCLK / 15 (3.579545 MHz on NTSC). An internal ÷16
//! prescaler clocks the tone and noise counters, so a tone with period N
//! toggles every 16·N PSG clocks and sounds at `PSG_clock / (32·N)`.
//!
//! Integrated (Sega) variant details, matching Genesis Plus GX `psg.c`
//! (`PSG_MCYCLES_RATIO = 15*16`, integrated `zeroFreqInc`, 16-bit noise
//! register with `noiseBitMask = 0x9`) and MAME `segapsg_device`
//! (feedback mask 0x8000, taps 0x01/0x08):
//! - a tone period of 0 behaves as period 1;
//! - the noise LFSR is 16 bits wide, seeded with 0x8000, white-noise
//!   feedback is the parity of bits 0 and 3 (taps 0x0009), shifted into bit
//!   15; periodic noise therefore has a period of 16;
//! - the noise counter toggles a flip-flop on every reload and the LFSR
//!   shifts only on the flip-flop's rising edge; rate 3 follows tone 2's
//!   counter (GPGX copies tone 2's counter into the noise counter);
//! - a data byte while the noise register is latched rewrites the noise
//!   control bits and resets the LFSR (GPGX `psg_write`, MAME `write`).
//!
//! Output is synthesized as band-limited steps through [`BlipBuf`]. As in
//! GPGX, each channel is unipolar (0 or +volume); the DC this leaves is
//! removed by the APU's PSG DC blocker, standing in for the output
//! coupling capacitor.

use crate::apu::blip_buf::BlipBuf;
use crate::audio;
use serde::{Deserialize, Serialize};

fn default_master_clock() -> u32 {
    audio::NTSC_MCLK
}

fn default_sample_rate() -> u32 {
    audio::SAMPLE_RATE
}

fn default_lfsr() -> u16 {
    LFSR_SEED
}

/// Master clocks per PSG input clock (the PSG runs at MCLK / 15).
pub const MCLK_PER_PSG_CLOCK: u32 = 15;
/// The internal prescaler clocks the counters once every 16 PSG clocks.
pub const PSG_CLOCKS_PER_TICK: u32 = 16;
/// Master clocks per counter tick.
const MCLK_PER_TICK: u32 = MCLK_PER_PSG_CLOCK * PSG_CLOCKS_PER_TICK;
/// Noise LFSR value after any write to the noise control register.
pub const LFSR_SEED: u16 = 0x8000;
/// White-noise feedback taps (bits 0 and 3) of the Sega 16-bit LFSR.
pub const WHITE_NOISE_TAPS: u16 = 0x0009;

/// Channel amplitude for each attenuation step (2 dB per step,
/// `4095 · 10^(-v/10)`, the same curve as before; 15 is off). The table
/// avoids evaluating a power function on every output transition.
pub(crate) const VOLUME_TABLE: [i32; 16] = [
    4095, 3253, 2584, 2052, 1630, 1295, 1029, 817, 649, 516, 410, 325, 258, 205, 163, 0,
];

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

impl ToneChannel {
    /// Effective counter reload value: period 0 behaves as period 1.
    fn period(&self) -> u16 {
        self.frequency.max(1)
    }

    fn amplitude(&self) -> i32 {
        if self.output {
            VOLUME_TABLE[(self.volume & 0x0F) as usize]
        } else {
            0
        }
    }
}

/// Noise channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseChannel {
    pub white_noise: bool,
    pub shift_rate: u8,
    pub volume: u8,
    #[serde(default = "default_lfsr")]
    pub lfsr: u16,
    pub counter: u16,
    /// Divider flip-flop toggled on each counter reload; the LFSR shifts on
    /// its rising edge.
    #[serde(default)]
    pub flip_flop: bool,
    pub last_amp: i32,
}

impl Default for NoiseChannel {
    fn default() -> Self {
        Self {
            white_noise: false,
            shift_rate: 0,
            volume: 0x0F,
            lfsr: LFSR_SEED,
            counter: 0,
            flip_flop: false,
            last_amp: 0,
        }
    }
}

impl NoiseChannel {
    fn amplitude(&self) -> i32 {
        if (self.lfsr & 1) != 0 {
            VOLUME_TABLE[(self.volume & 0x0F) as usize]
        } else {
            0
        }
    }

    /// Fixed counter reload for rates 0-2; rate 3 follows tone 2 instead.
    fn fixed_period(&self) -> Option<u16> {
        match self.shift_rate & 3 {
            0 => Some(0x10),
            1 => Some(0x20),
            2 => Some(0x40),
            _ => None,
        }
    }

    fn shift_lfsr(&mut self) {
        let feedback = if self.white_noise {
            (self.lfsr & WHITE_NOISE_TAPS).count_ones() as u16 & 1
        } else {
            self.lfsr & 1
        };
        self.lfsr = (self.lfsr >> 1) | (feedback << 15);
    }

    fn write_control(&mut self, data: u8) {
        self.white_noise = (data & 0x04) != 0;
        self.shift_rate = data & 0x03;
        self.lfsr = LFSR_SEED;
    }
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
    /// Master clock of the most recent counter tick
    pub total_mclocks: u64,
    /// Master clocks accumulated towards the next counter tick
    pub mclk_debt: u32,
    /// Band-limited synthesis buffer
    pub blip: BlipBuf,
}

impl Psg {
    pub fn new() -> Self {
        let mut psg = Self {
            tones: std::array::from_fn(|_| ToneChannel::default()),
            noise: NoiseChannel::default(),
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

    fn current_mclk(&self) -> u64 {
        self.total_mclocks.wrapping_add(self.mclk_debt as u64)
    }

    /// Reconfigure PSG timing for the active video region and output sample rate.
    ///
    /// The band-limited buffer keeps its settled level, so each channel's
    /// last reported amplitude remains a valid baseline for later deltas.
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
        let now = self.current_mclk();
        self.blip.set_timing(master_clock, sample_rate, now);
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
        let (new_amp, last_amp) = match channel {
            0..=2 => {
                let t = &mut self.tones[channel as usize];
                let new_amp = t.amplitude();
                let last = std::mem::replace(&mut t.last_amp, new_amp);
                (new_amp, last)
            }
            3 => {
                let new_amp = self.noise.amplitude();
                let last = std::mem::replace(&mut self.noise.last_amp, new_amp);
                (new_amp, last)
            }
            _ => return,
        };
        self.blip.add_delta(clock, new_amp - last_amp);
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
            let tone = &mut self.tones[channel as usize];
            tone.frequency = (tone.frequency & 0x3F0) | (data as u16);
            self.update_channel_amp(channel);
        } else {
            self.noise.write_control(data);
            self.update_channel_amp(3);
        }
    }

    fn write_frequency_high(&mut self, channel: u8, data: u8) {
        if channel < 3 {
            let tone = &mut self.tones[channel as usize];
            tone.frequency = (tone.frequency & 0x00F) | ((data as u16) << 4);
            self.update_channel_amp(channel);
        } else {
            /* A data byte while the noise register is latched rewrites the
             * noise control bits and resets the LFSR, like a latch write. */
            self.noise.write_control(data & 0x07);
            self.update_channel_amp(3);
        }
    }

    /// One prescaled counter tick (every 16 PSG clocks).
    fn tick(&mut self) {
        self.total_mclocks = self.total_mclocks.wrapping_add(MCLK_PER_TICK as u64);

        let mut tone2_reloaded = false;
        for i in 0..3 {
            let tone = &mut self.tones[i];
            tone.counter = tone.counter.saturating_sub(1);
            if tone.counter == 0 {
                tone.counter = tone.period();
                tone.output = !tone.output;
                if i == 2 {
                    tone2_reloaded = true;
                }
                self.update_channel_amp(i as u8);
            }
        }

        let noise_event = match self.noise.fixed_period() {
            Some(period) => {
                self.noise.counter = self.noise.counter.saturating_sub(1);
                if self.noise.counter == 0 {
                    self.noise.counter = period;
                    true
                } else {
                    false
                }
            }
            None => tone2_reloaded,
        };
        if noise_event {
            self.noise.flip_flop = !self.noise.flip_flop;
            if self.noise.flip_flop {
                self.noise.shift_lfsr();
                self.update_channel_amp(3);
            }
        }
    }

    fn run_mclk(&mut self, mclk: u64) {
        let mut debt = self.mclk_debt as u64 + mclk;
        while debt >= MCLK_PER_TICK as u64 {
            debt -= MCLK_PER_TICK as u64;
            self.tick();
        }
        self.mclk_debt = debt as u32;
    }

    /// Step the PSG by a number of PSG input clocks (MCLK / 15).
    pub fn step_cycles(&mut self, cycles: u32) {
        self.run_mclk(cycles as u64 * MCLK_PER_PSG_CLOCK as u64);
    }

    /// Step the PSG using M68K cycles from the system bus.
    pub fn step_m68k_cycles(&mut self, cycles: u32) {
        self.run_mclk(cycles as u64 * 7);
    }

    pub fn current_sample(&self) -> i16 {
        let mut out = 0i32;
        for tone in &self.tones {
            out += tone.last_amp;
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

    /// Step the PSG by one PSG clock and generate a sample (legacy)
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
        assert_eq!(psg.noise.lfsr, 0x8000);
        assert_eq!(psg.latch_channel, 0);
        assert!(!psg.latch_volume);
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
        assert!(!psg.latch_volume);
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
        assert!(psg.latch_volume);
        assert_eq!(psg.tones[1].volume, 4);

        // Data write to volume latch
        // 0000_0011 => Latch=0, Data=0011
        psg.write(0x03);
        assert_eq!(psg.tones[1].volume, 3);
    }

    #[test]
    fn test_psg_step_m68k_cycles_accumulates_prescaler_debt() {
        let mut psg = Psg::new();
        psg.step_m68k_cycles(1);
        assert_eq!(psg.mclk_debt, 7);
        psg.step_m68k_cycles(33);
        assert_eq!(psg.mclk_debt, 238);
        // 238 + 7 = 245 → one 240-MCLK counter tick, 5 left over.
        psg.step_m68k_cycles(1);
        assert_eq!(psg.mclk_debt, 5);
        assert_eq!(psg.total_mclocks, 240);
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

    #[test]
    fn test_volume_table_matches_2db_steps() {
        for v in 0..15u8 {
            let expected = 4095.0 * 10f64.powf(-(v as f64) / 10.0);
            assert!((VOLUME_TABLE[v as usize] as f64 - expected).abs() <= 0.5);
        }
        assert_eq!(VOLUME_TABLE[15], 0);
    }
}
