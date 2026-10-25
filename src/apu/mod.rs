//! Audio Processing Unit (APU)
//!
//! Refactored to use band-limited synthesis for both FM and PSG.

pub mod blip_buf;
pub mod psg;
pub(crate) mod state_clamp;
pub mod ym2612;

#[cfg(test)]
mod tests_blip_buf;
#[cfg(test)]
mod tests_psg_expansion;
#[cfg(test)]
mod tests_ym2612_expansion;
#[cfg(test)]
mod tests_ym2612_unit;

use crate::debugger::Debuggable;
use psg::Psg;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ym2612::{Bank, Ym2612};

fn default_region() -> Region {
    Region::Ntsc
}

fn default_sample_rate() -> u32 {
    crate::audio::SAMPLE_RATE
}

fn default_lpf_b0() -> f32 {
    low_pass_coefficients(crate::audio::SAMPLE_RATE).0
}

fn default_lpf_b1() -> f32 {
    low_pass_coefficients(crate::audio::SAMPLE_RATE).1
}

fn default_lpf_a1() -> f32 {
    low_pass_coefficients(crate::audio::SAMPLE_RATE).2
}

fn default_psg_dc_pole() -> f32 {
    dc_blocker_pole(crate::audio::SAMPLE_RATE)
}

/// Corner frequency of the PSG DC-blocking high-pass (AC coupling).
const PSG_DC_BLOCK_HZ: f32 = 20.0;

/// Pole of the one-pole DC blocker `y = x - x' + R·y'`, `R = 1 - 2π·fc/fs`.
fn dc_blocker_pole(sample_rate: u32) -> f32 {
    let fs = sample_rate.max(1) as f32;
    (1.0 - 2.0 * std::f32::consts::PI * PSG_DC_BLOCK_HZ / fs).clamp(0.0, 1.0)
}

fn low_pass_coefficients(sample_rate: u32) -> (f32, f32, f32) {
    let cutoff_hz = 3390.0_f32;
    let sample_rate = sample_rate.max(1) as f32;
    let k = (std::f32::consts::PI * cutoff_hz / sample_rate).tan();
    let norm = 1.0_f32 / (1.0_f32 + k);
    let b0 = k * norm;
    let b1 = b0;
    let a1 = (k - 1.0_f32) * norm;
    (b0, b1, a1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Region {
    #[default]
    Ntsc,
    Pal,
}

impl Region {
    pub fn master_clock(self) -> u32 {
        match self {
            Region::Ntsc => crate::audio::NTSC_MCLK,
            Region::Pal => crate::audio::PAL_MCLK,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Apu {
    pub psg: Psg,
    pub fm: Ym2612,
    #[serde(default = "default_region")]
    pub region: Region,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_lpf_b0")]
    low_pass_b0: f32,
    #[serde(default = "default_lpf_b1")]
    low_pass_b1: f32,
    #[serde(default = "default_lpf_a1")]
    low_pass_a1: f32,
    #[serde(default)]
    low_pass_left: f32,
    #[serde(default)]
    low_pass_right: f32,
    #[serde(default)]
    low_pass_prev_input_left: f32,
    #[serde(default)]
    low_pass_prev_input_right: f32,
    /* The PSG channels are unipolar (0 or +volume, as in GPGX), and the
     * output stage is AC coupled: the DC blocker removes the level a tone,
     * a channel parked at period 0/1 (PCM playback) or a volume change
     * would otherwise leave on the mix. */
    #[serde(default = "default_psg_dc_pole")]
    psg_dc_pole: f32,
    #[serde(default)]
    psg_dc_prev_input: f32,
    #[serde(default)]
    psg_dc_prev_output: f32,
    #[serde(skip, default = "default_channel_buffers")]
    pub channel_buffers: [[i16; 128]; 10],
    #[serde(skip)]
    pub buffer_idx: usize,
}

fn default_channel_buffers() -> [[i16; 128]; 10] {
    [[0i16; 128]; 10]
}

impl Apu {
    fn mix_sample(sample: i32) -> i16 {
        sample.clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }

    pub fn new() -> Self {
        let mut apu = Self {
            psg: Psg::new(),
            fm: Ym2612::new(),
            region: Region::Ntsc,
            sample_rate: crate::audio::SAMPLE_RATE,
            low_pass_b0: default_lpf_b0(),
            low_pass_b1: default_lpf_b1(),
            low_pass_a1: default_lpf_a1(),
            low_pass_left: 0.0,
            low_pass_right: 0.0,
            low_pass_prev_input_left: 0.0,
            low_pass_prev_input_right: 0.0,
            psg_dc_pole: default_psg_dc_pole(),
            psg_dc_prev_input: 0.0,
            psg_dc_prev_output: 0.0,
            channel_buffers: [[0; 128]; 10],
            buffer_idx: 0,
        };
        apu.set_timing(Region::Ntsc, crate::audio::SAMPLE_RATE);
        apu
    }

    pub fn reset(&mut self) {
        self.psg.reset();
        self.fm.reset();
        self.low_pass_left = 0.0;
        self.low_pass_right = 0.0;
        self.low_pass_prev_input_left = 0.0;
        self.low_pass_prev_input_right = 0.0;
        self.psg_dc_prev_input = 0.0;
        self.psg_dc_prev_output = 0.0;
    }

    /// Configure the APU timing for the current region and output sample rate.
    pub fn set_timing(&mut self, region: Region, sample_rate: u32) {
        let master_clock = region.master_clock();
        if self.region == region
            && self.sample_rate == sample_rate
            && self.psg.master_clock == master_clock
            && self.psg.sample_rate == sample_rate
            && self.psg.blip.clock_rate() == master_clock
            && self.psg.blip.sample_rate() == sample_rate
            && self.fm.master_clock == master_clock
            && self.fm.sample_rate == sample_rate
            && self.fm.blip_l.clock_rate() == master_clock
            && self.fm.blip_l.sample_rate() == sample_rate
            && self.fm.blip_r.clock_rate() == master_clock
            && self.fm.blip_r.sample_rate() == sample_rate
        {
            return;
        }
        self.region = region;
        self.sample_rate = sample_rate;
        self.psg.set_timing(master_clock, sample_rate);
        self.fm.set_timing(master_clock, sample_rate);
        let (b0, b1, a1) = low_pass_coefficients(sample_rate);
        self.low_pass_b0 = b0;
        self.low_pass_b1 = b1;
        self.low_pass_a1 = a1;
        self.psg_dc_pole = dc_blocker_pole(sample_rate);
        self.low_pass_left = 0.0;
        self.low_pass_right = 0.0;
        self.low_pass_prev_input_left = 0.0;
        self.low_pass_prev_input_right = 0.0;
    }

    /// Update only the active video region while preserving the current output rate.
    pub fn set_region(&mut self, region: Region) {
        self.set_timing(region, self.sample_rate);
    }

    /// Update only the output sample rate while preserving the current region.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.set_timing(self.region, sample_rate);
    }

    pub fn write_psg(&mut self, data: u8) {
        self.psg.write(data);
    }

    pub fn read_fm_status(&self) -> u8 {
        self.fm.read_status()
    }

    pub fn write_fm_addr(&mut self, bank: Bank, data: u8) {
        self.fm.write_addr(bank, data);
    }

    /// Write the FM data port. As on hardware there is a single data port:
    /// the write targets the register group selected by the most recent
    /// address-port write.
    pub fn write_fm_data(&mut self, data: u8) {
        self.fm.write_data(0, data);
    }

    pub fn tick_cycles(&mut self, m68k_cycles: u32) {
        self.fm.step(m68k_cycles);
        self.psg.step_m68k_cycles(m68k_cycles);
    }

    /// Attempts to generate a mixed audio sample pair.
    /// Returns `(left, right)` from the blip buffers.
    pub fn generate_sample(&mut self) -> (i16, i16) {
        let (fm_l, fm_r) = self.fm.generate_sample();
        let psg_raw = self.psg.generate_sample();
        let psg = self.psg_dc_block(psg_raw);

        let dry_left = ((fm_l as f32) * 3.0 + psg * 2.0) / 5.0;
        let dry_right = ((fm_r as f32) * 3.0 + psg * 2.0) / 5.0;

        let filtered_left = self.low_pass_b0 * dry_left
            + self.low_pass_b1 * self.low_pass_prev_input_left
            - self.low_pass_a1 * self.low_pass_left;
        let filtered_right = self.low_pass_b0 * dry_right
            + self.low_pass_b1 * self.low_pass_prev_input_right
            - self.low_pass_a1 * self.low_pass_right;

        self.low_pass_prev_input_left = dry_left;
        self.low_pass_prev_input_right = dry_right;
        self.low_pass_left = filtered_left;
        self.low_pass_right = filtered_right;

        let left = Self::mix_sample(self.low_pass_left.round() as i32);
        let right = Self::mix_sample(self.low_pass_right.round() as i32);

        (left, right)
    }

    /// One-pole DC blocker applied to the PSG before mixing.
    fn psg_dc_block(&mut self, sample: i16) -> f32 {
        let x = sample as f32;
        let y = x - self.psg_dc_prev_input + self.psg_dc_pole * self.psg_dc_prev_output;
        self.psg_dc_prev_input = x;
        self.psg_dc_prev_output = if y.is_finite() { y } else { 0.0 };
        self.psg_dc_prev_output
    }

    /// Update visualization buffers (call once per frame)
    pub fn update_visualization(&mut self) {
        let fm_samples = self.fm.generate_channel_samples();
        let psg_samples = self.psg.get_channel_samples();
        for (i, sample) in fm_samples.iter().enumerate() {
            self.channel_buffers[i][self.buffer_idx] = *sample;
        }
        for (i, sample) in psg_samples.iter().enumerate() {
            self.channel_buffers[6 + i][self.buffer_idx] = *sample;
        }
        self.buffer_idx = (self.buffer_idx + 1) % 128;
    }
}

impl Debuggable for Apu {
    fn read_state(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }

    fn write_state(&mut self, state: &Value) {
        if let Ok(new_apu) = Apu::deserialize(state) {
            *self = new_apu;
        }
    }
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio;

    #[test]
    fn test_initialization() {
        let apu = Apu::new();
        assert_eq!(apu.psg.tones[0].volume, 0x0F);
        assert_eq!(apu.fm.status, 0);
    }

    #[test]
    fn test_psg_passthrough() {
        let mut apu = Apu::new();
        apu.write_psg(0x8F);
        apu.write_psg(0x90);
        assert_eq!(apu.psg.tones[0].volume, 0);
    }

    #[test]
    fn test_fm_passthrough() {
        let mut apu = Apu::new();
        apu.write_fm_addr(Bank::Bank0, 0x28);
        apu.write_fm_data(0xF0);
        assert_eq!(apu.fm.registers[0][0x28], 0xF0);
    }

    #[test]
    fn test_mix_sample_is_linear_in_range() {
        assert_eq!(Apu::mix_sample(12345), 12345);
        assert_eq!(Apu::mix_sample(-12345), -12345);
    }

    #[test]
    fn test_mix_sample_clamps_instead_of_soft_clipping() {
        assert_eq!(Apu::mix_sample(40_000), i16::MAX);
        assert_eq!(Apu::mix_sample(-40_000), i16::MIN);
    }

    #[test]
    fn test_weighted_mix_is_normalized() {
        let mixed = ((20_000i32 * 3) + (20_000i32 * 2)) / 5;
        assert_eq!(mixed, 20_000);
    }

    #[test]
    fn test_low_pass_coefficients_match_reference_design() {
        let native_rate = (53_693_175.0_f32 / 7.0_f32 / 6.0_f32 / 24.0_f32).round() as u32;
        let (b0, b1, a1) = low_pass_coefficients(native_rate);
        assert!((b0 - 0.16849834).abs() < 1e-6);
        assert!((b1 - 0.16849834).abs() < 1e-6);
        assert!((a1 + 0.6630033).abs() < 1e-6);
    }

    #[test]
    fn test_low_pass_filter_smooths_step_changes() {
        let mut apu = Apu::new();
        apu.low_pass_left = 0.0;
        apu.low_pass_right = 0.0;
        apu.low_pass_prev_input_left = 0.0;
        apu.low_pass_prev_input_right = 0.0;

        let dry = 20_000.0_f32;
        apu.low_pass_left = apu.low_pass_b0 * dry + apu.low_pass_b1 * apu.low_pass_prev_input_left
            - apu.low_pass_a1 * apu.low_pass_left;
        apu.low_pass_right = apu.low_pass_b0 * dry
            + apu.low_pass_b1 * apu.low_pass_prev_input_right
            - apu.low_pass_a1 * apu.low_pass_right;

        assert!(apu.low_pass_left > 0.0);
        assert!(apu.low_pass_left < dry);
        assert_eq!(apu.low_pass_left, apu.low_pass_right);
    }

    #[test]
    fn test_set_timing_noops_when_configuration_is_unchanged() {
        let mut apu = Apu::new();
        apu.low_pass_left = 12.5;
        apu.low_pass_right = -3.0;
        apu.low_pass_prev_input_left = 9.0;
        apu.low_pass_prev_input_right = -4.0;

        apu.set_timing(Region::Ntsc, crate::audio::SAMPLE_RATE);

        assert_eq!(apu.low_pass_left, 12.5);
        assert_eq!(apu.low_pass_right, -3.0);
        assert_eq!(apu.low_pass_prev_input_left, 9.0);
        assert_eq!(apu.low_pass_prev_input_right, -4.0);
    }

    #[test]
    fn test_write_fm_addr() {
        let mut apu = Apu::new();

        // Write address to Bank0, and check if data written goes to this address
        apu.write_fm_addr(Bank::Bank0, 0x22);
        apu.write_fm_data(0x11);
        assert_eq!(apu.fm.registers[0][0x22], 0x11);
        apu.tick_cycles(32);

        // Change address in Bank0, write data, check new address is used
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.tick_cycles(32);
        apu.write_fm_data(0x33);
        assert_eq!(apu.fm.registers[0][0x27], 0x33);
        apu.tick_cycles(32);

        // Verify that Bank1 operates independently
        apu.write_fm_addr(Bank::Bank1, 0x28);
        apu.tick_cycles(32);
        apu.write_fm_data(0x44);
        assert_eq!(apu.fm.registers[1][0x28], 0x44);
        apu.tick_cycles(32);

        // Change address in Bank1
        apu.write_fm_addr(Bank::Bank1, 0x2B);
        apu.tick_cycles(32);
        apu.write_fm_data(0x55);
        assert_eq!(apu.fm.registers[1][0x2B], 0x55);
        apu.tick_cycles(32);

        // The single data port follows the last address write (Bank1 $2B),
        // leaving Bank0's registers untouched.
        apu.write_fm_data(0x66);
        assert_eq!(apu.fm.registers[1][0x2B], 0x66);
        assert_eq!(apu.fm.registers[0][0x27], 0x33);

        // Re-selecting Bank0 routes data there again.
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.write_fm_data(0x77);
        assert_eq!(apu.fm.registers[0][0x27], 0x77);
    }

    #[test]
    fn test_write_fm_data() {
        let mut apu = Apu::new();

        // Write to Bank0, Register 0x24 (Timer A High)
        apu.write_fm_addr(Bank::Bank0, 0x24);
        apu.write_fm_data(0xAA);
        assert_eq!(apu.fm.registers[0][0x24], 0xAA);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag should be set

        apu.tick_cycles(32); // clear busy

        // Write to Bank1, Register 0x24
        apu.write_fm_addr(Bank::Bank1, 0x24);
        apu.tick_cycles(32);
        apu.write_fm_data(0xBB);
        assert_eq!(apu.fm.registers[1][0x24], 0xBB);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag should be set
    }

    #[test]
    fn test_write_fm_data_applies_immediately() {
        // FM data writes apply immediately. Modelling the hardware busy-drop
        // would require cycle-exact write timing; because the FM is stepped in
        // batches, a busy gate spuriously drops the back-to-back address/data
        // writes real games perform, silencing the chip. `busy` is retained only
        // for the status read.
        let mut apu = Apu::new();

        apu.write_fm_addr(Bank::Bank0, 0x24);
        apu.write_fm_data(0xAA);
        assert_eq!(apu.fm.registers[0][0x24], 0xAA);

        apu.write_fm_addr(Bank::Bank0, 0x25);
        apu.write_fm_data(0x03);
        assert_eq!(apu.fm.registers[0][0x25], 0x03);
    }

    #[test]
    fn test_apu_write_fm_data_delegation_side_effects() {
        let mut apu = Apu::new();

        // 1. Test DAC Enable (Bank0, Register 0x2B)
        apu.write_fm_addr(Bank::Bank0, 0x2B);
        apu.write_fm_data(0x80); // Enable DAC
        assert_eq!(apu.fm.registers[0][0x2B], 0x80);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag set
        apu.tick_cycles(32); // clear busy

        // 2. Test DAC Value (Bank0, Register 0x2A)
        apu.write_fm_addr(Bank::Bank0, 0x2A);
        apu.write_fm_data(0xFF); // Set DAC value to maximum
        assert_eq!(apu.fm.registers[0][0x2A], 0xFF);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag set
        apu.tick_cycles(32); // clear busy

        // 3. Test Panning Update (Bank1, Register 0xB6)
        apu.write_fm_addr(Bank::Bank1, 0xB6);
        apu.write_fm_data(0xC0); // Left and Right panning
        assert_eq!(apu.fm.registers[1][0xB6], 0xC0);
        assert!((apu.read_fm_status() & 0x80) != 0); // Busy flag set
    }

    #[test]
    fn test_read_fm_status() {
        let mut apu = Apu::new();
        // Initial status should be 0
        assert_eq!(apu.read_fm_status(), 0);

        // Writing FM data should set the busy bit (bit 7)
        apu.write_fm_addr(Bank::Bank0, 0x22);
        apu.tick_cycles(32);
        apu.write_fm_data(0);
        assert!((apu.read_fm_status() & 0x80) != 0);

        // Tick cycles to clear busy bit: busy is 32 internal YM cycles =
        // 1344 MCLK; mclks is cycles * 7, so 192 CPU cycles clear it.
        apu.tick_cycles(192);
        assert_eq!(apu.read_fm_status() & 0x80, 0);

        // Test Timer A
        // Timer A is set via 0x24 (bits 9-2) and 0x25 (bits 1-0)
        // Set it to a very small value to trigger quickly
        apu.write_fm_addr(Bank::Bank0, 0x24);
        apu.write_fm_data(0xFF);
        apu.write_fm_addr(Bank::Bank0, 0x25);
        apu.tick_cycles(32);
        apu.write_fm_data(0x03); // Max value is 1023 (0x3FF)

        // Enable and trigger timer A (bit 0 = enable, bit 2 = load bit)
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.tick_cycles(32);
        apu.write_fm_data(0x05);

        // After some cycles, bit 0 should be set
        // Timer A advances on native YM2612 sample periods.
        // One YM sample is 144 chip clocks = 1008 Genesis master clocks.
        // tick_cycles(144) advances exactly one YM sample period.
        apu.tick_cycles(144);
        assert!((apu.read_fm_status() & 0x01) != 0);

        // Test Timer B
        // Reset status (YM2612 reset status bits via register 0x27 bits 4 and 5)
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.tick_cycles(32);
        apu.write_fm_data(0x30);
        assert_eq!(apu.read_fm_status() & 0x03, 0);

        // Set Timer B to max value (255)
        apu.write_fm_addr(Bank::Bank0, 0x26);
        apu.tick_cycles(32);
        apu.write_fm_data(0xFF);

        // Enable and trigger Timer B (bit 1 = enable, bit 3 = load bit)
        apu.write_fm_addr(Bank::Bank0, 0x27);
        apu.tick_cycles(32);
        apu.write_fm_data(0x0A);

        // Timer B ticks once every 16 YM sample periods.
        // With the max timer value, overflow needs 16 sample periods of divider advance.
        apu.tick_cycles(2304 * 16);
        assert!((apu.read_fm_status() & 0x02) != 0);
    }

    #[test]
    fn test_write_fm_data_side_effects() {
        let mut apu = Apu::new();

        // Enable DAC
        apu.write_fm_addr(Bank::Bank0, 0x2B);
        apu.write_fm_data(0x80);

        // Set DAC value to a non-zero amplitude
        apu.write_fm_addr(Bank::Bank0, 0x2A);
        apu.write_fm_data(0xFF);

        // Pan Left Only to test specific output
        apu.write_fm_addr(Bank::Bank1, 0xB6);
        apu.write_fm_data(0x80);

        // Tick one native YM2612 sample period.
        apu.tick_cycles(144);

        // Assert DAC output is observable in the blip buffer
        assert!(
            apu.fm.blip_l.read_instant() > 0,
            "Left audio should be positive due to DAC"
        );
        assert_eq!(
            apu.fm.blip_r.read_instant(),
            (768 * 79 / 120) as i16, /* ladder baseline at the 79/120 output gain */
            "Right audio should contain only the YM2612 ladder-effect baseline"
        );
    }

    #[test]
    fn test_apu_set_timing_updates_all_subsystems() {
        let mut apu = Apu::new();

        apu.set_timing(Region::Pal, 48_000);

        assert_eq!(apu.region, Region::Pal);
        assert_eq!(apu.sample_rate, 48_000);
        assert_eq!(apu.psg.master_clock, audio::PAL_MCLK);
        assert_eq!(apu.fm.master_clock, audio::PAL_MCLK);
        assert_eq!(apu.psg.blip.clock_rate(), audio::PAL_MCLK);
        assert_eq!(apu.fm.blip_l.clock_rate(), audio::PAL_MCLK);
        assert_eq!(apu.psg.blip.sample_rate(), 48_000);
        assert_eq!(apu.fm.blip_l.sample_rate(), 48_000);
    }

    #[test]
    fn test_apu_set_timing_repairs_stale_child_timing() {
        let mut apu = Apu::new();

        apu.psg.master_clock = 1;
        apu.psg.sample_rate = 1;
        apu.fm.master_clock = 1;
        apu.fm.sample_rate = 1;

        apu.set_timing(Region::Ntsc, audio::SAMPLE_RATE);

        assert_eq!(apu.region, Region::Ntsc);
        assert_eq!(apu.sample_rate, audio::SAMPLE_RATE);
        assert_eq!(apu.psg.master_clock, audio::NTSC_MCLK);
        assert_eq!(apu.psg.sample_rate, audio::SAMPLE_RATE);
        assert_eq!(apu.fm.master_clock, audio::NTSC_MCLK);
        assert_eq!(apu.fm.sample_rate, audio::SAMPLE_RATE);
        assert_eq!(apu.psg.blip.clock_rate(), audio::NTSC_MCLK);
        assert_eq!(apu.fm.blip_l.clock_rate(), audio::NTSC_MCLK);
        assert_eq!(apu.fm.blip_r.clock_rate(), audio::NTSC_MCLK);
    }

    /// Run the APU like the frontend does: tick, then emit output samples.
    fn run_samples(apu: &mut Apu, samples: usize) -> Vec<(i16, i16)> {
        let cps = audio::NTSC_MCLK as f64 / apu.sample_rate as f64;
        let mut acc = 0.0f64;
        let mut out = Vec::with_capacity(samples);
        while out.len() < samples {
            apu.tick_cycles(64);
            acc += 64.0 * 7.0;
            while acc >= cps && out.len() < samples {
                out.push(apu.generate_sample());
                acc -= cps;
            }
        }
        out
    }

    fn psg_only_mean(samples: &[(i16, i16)]) -> f64 {
        samples.iter().map(|&(l, _)| l as f64).sum::<f64>() / samples.len() as f64
    }

    #[test]
    fn test_psg_volume_change_leaves_no_dc_on_mix() {
        let mut apu = Apu::new();
        /* FM fully silent (YM3438 profile: no ladder offset). */
        apu.fm
            .set_hardware_profile(ym2612::Ym2612HardwareProfile::Ym3438);
        let rate = apu.sample_rate as usize;
        run_samples(&mut apu, rate / 10);
        /* A held channel (period 0/1 toggles ultrasonically) at full volume
         * is a DC level, as when a game parks a channel or plays PCM. */
        apu.write_psg(0x80);
        apu.write_psg(0x00);
        apu.write_psg(0x90);
        let out = run_samples(&mut apu, rate * 2);
        let mean = psg_only_mean(&out[rate..]);
        assert!(mean.abs() < 20.0, "PSG left DC {mean} on the mix");
    }

    #[test]
    fn test_psg_tone_mix_is_centred() {
        let mut apu = Apu::new();
        apu.fm
            .set_hardware_profile(ym2612::Ym2612HardwareProfile::Ym3438);
        let rate = apu.sample_rate as usize;
        apu.write_psg(0x8E); // tone 0 period 0x0FE (~440 Hz)
        apu.write_psg(0x0F);
        apu.write_psg(0x90);
        let out = run_samples(&mut apu, rate * 2);
        let mean = psg_only_mean(&out[rate..]);
        let peak = out[rate..].iter().map(|&(l, _)| l.abs()).max().unwrap();
        assert!(peak > 500, "tone should be audible, peak {peak}");
        assert!(mean.abs() < 20.0, "tone mix not centred: {mean}");
    }

    #[test]
    fn test_region_change_keeps_delta_baselines() {
        let mut apu = Apu::new();
        apu.write_psg(0x80);
        apu.write_psg(0x00);
        apu.write_psg(0x90); // channel 0 audible
        run_samples(&mut apu, 2000);
        let level = apu.psg.blip.read_instant();

        apu.set_timing(Region::Pal, 48_000);
        assert_eq!(apu.psg.blip.read_instant(), level);

        apu.write_psg(0x9F); // silence everything
        run_samples(&mut apu, 500);
        assert_eq!(apu.psg.blip.read_instant(), 0, "PSG baseline offset");
        let mut last = 1;
        for _ in 0..64 {
            last = apu.psg.generate_sample();
        }
        assert_eq!(last, 0);
        let fm_settled = apu.fm.blip_l.read_instant();
        let mut fm_last = (0, 0);
        for _ in 0..64 {
            fm_last = apu.fm.generate_sample();
        }
        assert_eq!(fm_last.0, fm_settled, "FM baseline offset");
    }

    #[test]
    fn test_write_fm_data_follows_last_address_bank() {
        let mut apu = Apu::new();
        apu.write_fm_addr(Bank::Bank1, 0x30);
        apu.write_fm_data(0x5A);
        assert_eq!(apu.fm.registers[1][0x30], 0x5A);
        assert_eq!(apu.fm.registers[0][0x30], 0x00);
    }

    /// Paths of every numeric leaf in a JSON value.
    fn numeric_leaves(v: &Value, path: String, out: &mut Vec<String>) {
        match v {
            Value::Number(_) => out.push(path),
            Value::Array(a) => {
                for (i, e) in a.iter().enumerate() {
                    numeric_leaves(e, format!("{path}/{i}"), out);
                }
            }
            Value::Object(o) => {
                for (k, e) in o {
                    numeric_leaves(e, format!("{path}/{k}"), out);
                }
            }
            _ => {}
        }
    }

    const NASTY: [i64; 22] = [
        0,
        1,
        3,
        7,
        8,
        15,
        16,
        23,
        24,
        31,
        32,
        127,
        255,
        256,
        1023,
        4095,
        65535,
        -1,
        i32::MAX as i64,
        i32::MIN as i64,
        u32::MAX as i64,
        i64::MAX,
    ];

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]
        /// Restoring APU state with arbitrary out-of-range numeric fields
        /// either rejects the state or clamps it; running afterwards never
        /// panics.
        #[test]
        fn prop_corrupted_state_never_panics(
            edits in proptest::collection::vec((proptest::prelude::any::<usize>(), 0usize..22), 1..24)
        ) {
            let mut apu = Apu::new();
            apu.write_psg(0x8F);
            apu.write_psg(0x90);
            let mut state = apu.read_state();
            let mut leaves = Vec::new();
            numeric_leaves(&state, String::new(), &mut leaves);
            for (pick, value) in &edits {
                let path = &leaves[pick % leaves.len()];
                if let Some(slot) = state.pointer_mut(path) {
                    *slot = serde_json::json!(NASTY[*value]);
                }
            }
            apu.write_state(&state);
            apu.set_timing(Region::Ntsc, crate::audio::SAMPLE_RATE);
            for _ in 0..200 {
                apu.tick_cycles(488);
                apu.generate_sample();
            }
            apu.update_visualization();
            apu.fm.read(0);
            apu.fm.read(1);
        }
    }
}
