use super::psg::{Psg, LFSR_SEED, PSG_CLOCKS_PER_TICK};
use crate::audio::NTSC_MCLK;
use proptest::prelude::*;

/// Advance the PSG by whole counter ticks (16 PSG clocks each).
fn tick(psg: &mut Psg, ticks: u32) {
    psg.step_cycles(ticks * PSG_CLOCKS_PER_TICK);
}

fn set_tone(psg: &mut Psg, channel: u8, period: u16, volume: u8) {
    psg.write(0x80 | (channel << 5) | (period & 0x0F) as u8);
    psg.write(((period >> 4) & 0x3F) as u8);
    psg.write(0x90 | (channel << 5) | (volume & 0x0F));
}

/// Ticks between consecutive output toggles of tone `channel`.
fn toggle_interval(psg: &mut Psg, channel: usize) -> u32 {
    /* Sync to a toggle first, then time one full half-period. */
    let start = psg.tones[channel].output;
    while psg.tones[channel].output == start {
        tick(psg, 1);
    }
    let start = psg.tones[channel].output;
    let mut ticks = 0;
    while psg.tones[channel].output == start {
        tick(psg, 1);
        ticks += 1;
    }
    ticks
}

#[test]
fn test_tone_frequency_includes_prescaler() {
    /* N = 254 at NTSC: 3579545 / (32 × 254) = 440.4 Hz. */
    let mut psg = Psg::new();
    set_tone(&mut psg, 0, 254, 0);
    let mut flips = 0u32;
    let mut last = psg.tones[0].output;
    /* One second of 68000 cycles (MCLK / 7), in scanline-sized chunks. */
    let total_m68k = NTSC_MCLK / 7;
    let mut done = 0;
    while done < total_m68k {
        let step = 488.min(total_m68k - done);
        psg.step_m68k_cycles(step);
        done += step;
        if psg.tones[0].output != last {
            flips += 1;
            last = psg.tones[0].output;
        }
    }
    let hz = flips as f64 / 2.0;
    let expected = (NTSC_MCLK as f64 / 15.0) / (32.0 * 254.0);
    assert!(
        (hz - expected).abs() / expected < 0.005,
        "measured {hz} Hz, expected {expected} Hz"
    );
    assert!((hz - 440.0).abs() / 440.0 < 0.005);
}

#[test]
fn test_tone_toggle_and_amplitude_are_unipolar() {
    let mut psg = Psg::new();
    set_tone(&mut psg, 0, 10, 0);
    assert_eq!(toggle_interval(&mut psg, 0), 10);
    let high = psg.tones[0].output;
    assert_eq!(psg.blip.read_instant(), if high { 4095 } else { 0 });
    tick(&mut psg, 10);
    assert_eq!(psg.blip.read_instant(), if high { 0 } else { 4095 });
}

#[test]
fn test_period_zero_toggles_like_period_one() {
    for period in [0u16, 1] {
        let mut psg = Psg::new();
        set_tone(&mut psg, 1, period, 0);
        assert_eq!(toggle_interval(&mut psg, 1), 1, "period {period}");
    }
}

#[test]
fn test_period_zero_counts_like_period_one() {
    /* Tone 2 at period 0 drives rate-3 noise exactly like period 1. */
    let run = |period: u16| {
        let mut psg = Psg::new();
        set_tone(&mut psg, 2, period, 15);
        psg.write(0xE3); // periodic noise following tone 2
        let mut lfsr = Vec::new();
        for _ in 0..64 {
            tick(&mut psg, 1);
            lfsr.push(psg.noise.lfsr);
        }
        lfsr
    };
    assert_eq!(run(0), run(1));
}

#[test]
fn test_all_tones_mixing() {
    let mut psg = Psg::new();
    set_tone(&mut psg, 0, 10, 0);
    set_tone(&mut psg, 1, 20, 2);
    set_tone(&mut psg, 2, 30, 4);
    for ch in 0..3u8 {
        psg.tones[ch as usize].output = true;
        psg.update_channel_amp(ch);
    }
    assert_eq!(psg.blip.read_instant() as i32, 4095 + 2584 + 1630);
    assert_eq!(psg.current_sample() as i32, 4095 + 2584 + 1630);
}

#[test]
fn test_noise_lfsr_seed_and_reset_on_write() {
    let mut psg = Psg::new();
    psg.write(0xE4); // white, rate 0
    assert_eq!(psg.noise.lfsr, LFSR_SEED);
    tick(&mut psg, 1000);
    assert_ne!(psg.noise.lfsr, LFSR_SEED);
    psg.write(0xE4);
    assert_eq!(psg.noise.lfsr, 0x8000);
}

/// Output bits of the noise channel over `shifts` LFSR shifts.
fn noise_bits(control: u8, shifts: usize) -> Vec<u16> {
    let mut psg = Psg::new();
    psg.write(0xE0 | control);
    let mut bits = Vec::with_capacity(shifts);
    /* Every shift changes the register (it never reaches 0 or a fixed point). */
    let mut last = psg.noise.lfsr;
    while bits.len() < shifts {
        tick(&mut psg, 1);
        if psg.noise.lfsr != last {
            bits.push(psg.noise.lfsr & 1);
            last = psg.noise.lfsr;
        }
    }
    bits
}

#[test]
fn test_periodic_noise_has_period_16() {
    let bits = noise_bits(0x00, 64);
    for i in 16..64 {
        assert_eq!(bits[i], bits[i - 16]);
    }
    assert_eq!(bits[..16].iter().filter(|&&b| b == 1).count(), 1);
}

/// Independent model of the Sega PSG white-noise LFSR (SMS Power notes).
fn reference_white_noise(len: usize) -> Vec<u16> {
    let mut reg: u32 = 0x8000;
    (0..len)
        .map(|_| {
            let bit0 = reg & 1;
            let bit3 = (reg >> 3) & 1;
            reg = (reg >> 1) | ((bit0 ^ bit3) << 15);
            (reg & 1) as u16
        })
        .collect()
}

#[test]
fn test_white_noise_matches_reference_sequence() {
    let bits = noise_bits(0x04, 2000);
    assert_eq!(bits, reference_white_noise(2000));
}

#[test]
fn test_white_noise_period_is_57337() {
    let mut psg = Psg::new();
    psg.write(0xE4);
    let mut shifts = 0u32;
    let mut first_after_start = None;
    let mut last = psg.noise.lfsr;
    while shifts < 120_000 {
        tick(&mut psg, 1);
        if psg.noise.lfsr != last {
            shifts += 1;
            last = psg.noise.lfsr;
            if shifts == 1 {
                first_after_start = Some(last);
            } else if Some(last) == first_after_start {
                assert_eq!(shifts - 1, 57337);
                return;
            }
        }
    }
    panic!("white noise did not repeat");
}

#[test]
fn test_noise_shifts_on_flip_flop_rising_edge_only() {
    let mut psg = Psg::new();
    psg.write(0xE4); // rate 0: counter reload 0x10
    let mut shift_ticks = Vec::new();
    let mut last = psg.noise.lfsr;
    for t in 1..=400u32 {
        tick(&mut psg, 1);
        if psg.noise.lfsr != last {
            assert!(psg.noise.flip_flop, "shift on falling edge at tick {t}");
            shift_ticks.push(t);
            last = psg.noise.lfsr;
        }
    }
    /* Reloads every 16 ticks, shifts on every second reload: 32 ticks. */
    for w in shift_ticks.windows(2) {
        assert_eq!(w[1] - w[0], 32);
    }
    assert!(shift_ticks.len() >= 10);
}

#[test]
fn test_noise_rate_3_locks_to_tone_2() {
    let mut psg = Psg::new();
    set_tone(&mut psg, 2, 50, 15);
    psg.write(0xE7); // white, rate 3
    let mut tone_reload_ticks = Vec::new();
    let mut shift_ticks = Vec::new();
    let (mut last_out, mut last_lfsr) = (psg.tones[2].output, psg.noise.lfsr);
    for t in 1..=2000u32 {
        tick(&mut psg, 1);
        if psg.tones[2].output != last_out {
            tone_reload_ticks.push(t);
            last_out = psg.tones[2].output;
        }
        if psg.noise.lfsr != last_lfsr {
            shift_ticks.push(t);
            last_lfsr = psg.noise.lfsr;
        }
    }
    /* Every LFSR shift coincides with a tone 2 reload, at half its rate. */
    assert!(shift_ticks.iter().all(|t| tone_reload_ticks.contains(t)));
    for w in shift_ticks.windows(2) {
        assert_eq!(w[1] - w[0], 100);
    }
}

#[test]
fn test_noise_data_byte_updates_control_and_resets_lfsr() {
    let mut psg = Psg::new();
    psg.write(0xE0); // latch noise: periodic, rate 0
    tick(&mut psg, 100); // 4 shifts of the 16-step periodic sequence
    assert_ne!(psg.noise.lfsr, LFSR_SEED);
    psg.write(0x05); // data byte while noise is latched
    assert!(psg.noise.white_noise);
    assert_eq!(psg.noise.shift_rate, 1);
    assert_eq!(psg.noise.lfsr, LFSR_SEED);
}

#[test]
fn test_noise_shift_rates() {
    for (rate, period) in [(0u8, 0x10u32), (1, 0x20), (2, 0x40)] {
        let mut psg = Psg::new();
        psg.write(0xE0 | rate);
        let mut last = psg.noise.lfsr;
        let mut shifts = Vec::new();
        for t in 1..=(period * 2 * 6) {
            tick(&mut psg, 1);
            if psg.noise.lfsr != last {
                shifts.push(t);
                last = psg.noise.lfsr;
            }
        }
        for w in shifts.windows(2) {
            assert_eq!(w[1] - w[0], 2 * period, "rate {rate}");
        }
    }
}

proptest! {
    /// Tone half-period equals N counter ticks (16·N PSG clocks) for every
    /// period, so the frequency is PSG_clock / (32·N).
    #[test]
    fn prop_tone_half_period_equals_n_ticks(n in 2u16..1024) {
        let mut psg = Psg::new();
        set_tone(&mut psg, 0, n, 0);
        prop_assert_eq!(toggle_interval(&mut psg, 0), n as u32);
    }

    /// Splitting the same elapsed time into arbitrary chunks yields the
    /// same PSG state (the prescaler keeps sub-tick remainders).
    #[test]
    fn prop_chunked_stepping_is_equivalent(
        chunks in prop::collection::vec(1u32..2000, 1..40),
        period in 1u16..1024,
    ) {
        let mut a = Psg::new();
        let mut b = Psg::new();
        set_tone(&mut a, 0, period, 3);
        set_tone(&mut b, 0, period, 3);
        a.write(0xE4);
        b.write(0xE4);
        let total: u32 = chunks.iter().sum();
        for c in &chunks {
            a.step_m68k_cycles(*c);
        }
        b.step_m68k_cycles(total);
        prop_assert_eq!(a.tones[0].output, b.tones[0].output);
        prop_assert_eq!(a.tones[0].counter, b.tones[0].counter);
        prop_assert_eq!(a.noise.lfsr, b.noise.lfsr);
        prop_assert_eq!(a.mclk_debt, b.mclk_debt);
    }
}
