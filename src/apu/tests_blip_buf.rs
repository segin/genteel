use super::blip_buf::{BlipBuf, KERNEL_CUTOFF};
use proptest::prelude::*;

const LATENCY: usize = BlipBuf::latency_samples();

fn read_n(blip: &mut BlipBuf, n: usize) -> Vec<i16> {
    let mut out = vec![0i16; n];
    let mut done = 0;
    while done < n {
        done += blip.read_samples(&mut out[done..]);
    }
    out
}

#[test]
fn test_initialization() {
    let blip = BlipBuf::new(3579545, 44100);
    assert_eq!(blip.read_instant(), 0);
}

#[test]
fn test_clear() {
    let mut blip = BlipBuf::new(3579545, 44100);
    blip.add_delta(0, 100);
    assert_eq!(blip.read_instant(), 100);
    blip.clear();
    assert_eq!(blip.read_instant(), 0);
}

#[test]
fn test_add_delta_and_read_instant() {
    let mut blip = BlipBuf::new(3579545, 44100);
    blip.add_delta(0, 500);
    assert_eq!(blip.read_instant(), 500);
    blip.add_delta(100, -200);
    assert_eq!(blip.read_instant(), 300);
    blip.add_delta(200, 40000); // Should clamp
    assert_eq!(blip.read_instant(), 32767);
}

#[test]
fn test_every_kernel_phase_has_unit_gain() {
    for phase in 0..256 {
        let k = BlipBuf::kernel_for_phase(phase);
        assert_eq!(k.iter().sum::<i32>(), 1 << 15, "phase {phase}");
    }
}

#[test]
fn test_step_settles_exactly_after_kernel() {
    let mut blip = BlipBuf::new(44100, 44100);
    blip.add_delta(0, 1000);
    let out = read_n(&mut blip, 32);
    /* Before the impulse centre the output only rings around 0; the step
     * lands on the sample `LATENCY` after its timestamp and then settles
     * exactly at the delta. */
    for &s in &out[..LATENCY] {
        assert!(s.abs() < 100, "{:?}", out);
    }
    assert!(out[LATENCY] > 500, "{:?}", out);
    for &s in &out[16..] {
        assert_eq!(s, 1000);
    }
}

#[test]
fn test_later_subsample_step_lands_later() {
    /* clock = 4 × sample rate: clock 1 → 0.25 samples, clock 3 → 0.75. */
    let step_at = |clock: u64| {
        let mut blip = BlipBuf::new(4 * 44100, 44100);
        blip.add_delta(clock, 10_000);
        read_n(&mut blip, 24)
    };
    let early = step_at(1);
    let late = step_at(3);
    /* Around the transition the earlier step is always further along. */
    for i in (LATENCY - 1)..=(LATENCY + 1) {
        assert!(
            early[i] > late[i],
            "sample {i}: early {} late {}",
            early[i],
            late[i]
        );
    }
    let sum = |v: &[i16]| v.iter().map(|&s| s as i64).sum::<i64>();
    /* Shifting a step later by 0.5 sample removes ~0.5 sample of area. */
    let area_diff = sum(&early) - sum(&late);
    assert!((area_diff - 5_000).abs() < 300, "area diff {area_diff}");
}

#[test]
fn test_delta_after_long_silence_lands_at_true_position() {
    let mut blip = BlipBuf::new(48_000, 48_000);
    /* Many frames with no deltas at all. */
    let silent = 48_000 * 3;
    let out = read_n(&mut blip, silent);
    assert!(out.iter().all(|&s| s == 0));
    /* Step at clock silent + 100 must appear 100 samples (+ latency) later,
     * not wherever stale relative bookkeeping would have put it. */
    blip.add_delta(silent as u64 + 100, 8_000);
    let out = read_n(&mut blip, 200);
    let crossing = out.iter().position(|&s| s >= 4_000).unwrap();
    assert!(
        (crossing as i64 - (100 + LATENCY) as i64).abs() <= 1,
        "crossed at {crossing}"
    );
}

#[test]
fn test_alternating_steps_do_not_drift_the_integrator() {
    /* Alternating +/-delta steps must keep tracking the square wave: the
     * taps of each step sum exactly to the delta. */
    let mclk = 53_693_175u32;
    let host = 48_000u32;
    let mut blip = BlipBuf::new(mclk, host);
    let cps = mclk as f64 / host as f64;

    let mut level = 0i32;
    let mut chip_i = 0u64;
    let mut clock = 0u64;
    let mut acc = 0.0f64;
    let mut last = 0i16;
    let mut sample = [0i16; 1];
    for _ in 0..400_000u32 {
        clock += 3416;
        while (chip_i + 1) * 1008 <= clock {
            chip_i += 1;
            if chip_i.is_multiple_of(24) {
                let new = if level == 2000 { -2000 } else { 2000 };
                blip.add_delta(chip_i * 1008, new - level);
                level = new;
            }
        }
        acc += 3416.0;
        while acc >= cps {
            blip.read_samples(&mut sample[..]);
            last = sample[0];
            acc -= cps;
        }
    }
    assert!(
        last.abs() < 4000,
        "band-limited output drifted to {last} instead of tracking +/-2000"
    );
}

#[test]
fn test_set_timing_keeps_settled_level() {
    let mut blip = BlipBuf::new(3579545, 44100);
    blip.add_delta(0, 500);
    read_n(&mut blip, 3);

    blip.set_timing(53203424, 48000, 123_456);

    assert_eq!(blip.clock_rate(), 53203424);
    assert_eq!(blip.sample_rate(), 48000);
    assert_eq!(blip.read_instant(), 500);
    /* The output continues at the settled level: no DC step. */
    assert!(read_n(&mut blip, 32).iter().all(|&s| s == 500));
    /* New deltas are timed against the new base clock. */
    blip.add_delta(123_456, -500);
    let out = read_n(&mut blip, 32);
    assert_eq!(*out.last().unwrap(), 0);
}

#[test]
fn test_set_timing_same_values_is_noop() {
    let mut blip = BlipBuf::new(3579545, 44100);
    blip.add_delta(0, 500);
    blip.set_timing(3579545, 44100, 99);
    assert_eq!(blip.clock_rate(), 3579545);
    assert_eq!(blip.sample_rate(), 44100);
    assert_eq!(blip.read_instant(), 500);
}

#[test]
fn test_long_running_delta_schedule_does_not_drift_out_of_window() {
    let rate = 53_267;
    let mut blip = BlipBuf::new(rate, rate);
    let mut sample = [0i16; 1];
    for clock in 0..(rate as u64 * 3) {
        blip.add_delta(clock, 1);
        assert_eq!(blip.read_samples(&mut sample), 1);
    }
    assert_eq!(blip.read_instant(), 32767);
}

#[test]
fn test_clear_resets_timebase() {
    let mut blip = BlipBuf::new(53_267, 53_267);
    let mut sample = [0i16; 1];
    blip.add_delta(10_000, 1000);
    assert_eq!(blip.read_samples(&mut sample), 1);
    blip.clear();
    blip.add_delta(0, 500);
    assert_eq!(blip.read_instant(), 500);
    let out = read_n(&mut blip, 32);
    assert_eq!(*out.last().unwrap(), 500);
}

/// Frequency response of the kernel at `freq` (fraction of the sample
/// rate), averaged over phases.
fn kernel_response(freq: f64) -> f64 {
    let mut total = 0.0;
    for phase in 0..256 {
        let k = BlipBuf::kernel_for_phase(phase);
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (n, &c) in k.iter().enumerate() {
            let w = 2.0 * std::f64::consts::PI * freq * n as f64;
            re += c as f64 * w.cos();
            im -= c as f64 * w.sin();
        }
        total += (re * re + im * im).sqrt() / (1 << 15) as f64;
    }
    total / 256.0
}

#[test]
fn test_kernel_cutoff_sits_below_nyquist() {
    assert!((0.85..=0.9).contains(&KERNEL_CUTOFF));
    /* Passband (well below the cutoff) is close to unity. */
    assert!((kernel_response(0.1) - 1.0).abs() < 0.05);
    /* At the design cutoff the response is about half (-6 dB). */
    let at_cutoff = kernel_response(KERNEL_CUTOFF * 0.5);
    assert!((0.3..0.7).contains(&at_cutoff), "{at_cutoff}");
    /* At Nyquist it is well attenuated. */
    assert!(kernel_response(0.5) < 0.2, "{}", kernel_response(0.5));
}

#[test]
fn test_restored_state_with_bad_buffer_is_repaired() {
    let mut blip = BlipBuf::new(48_000, 48_000);
    blip.add_delta(0, 700);
    let mut value = serde_json::to_value(&blip).unwrap();
    value["buffer"] = serde_json::json!([1, 2, 3]);
    value["start"] = serde_json::json!(999_999_999u64);
    value["read_pos"] = serde_json::json!(u64::MAX);
    let mut restored: BlipBuf = serde_json::from_value(value).unwrap();
    assert_eq!(restored.read_instant(), 700);
    restored.add_delta(u64::MAX, 5);
    let out = read_n(&mut restored, 64);
    assert_eq!(*out.last().unwrap(), 705);
}

#[test]
fn test_restored_state_with_zero_rates_is_rejected() {
    let blip = BlipBuf::new(48_000, 48_000);
    let mut value = serde_json::to_value(&blip).unwrap();
    value["clock_rate"] = serde_json::json!(0);
    assert!(serde_json::from_value::<BlipBuf>(value).is_err());
}

proptest! {
    /// Whatever the timing, the integrated output settles to the exact sum
    /// of all deltas (no amplitude is lost or invented).
    #[test]
    fn prop_integral_is_conserved(
        clock_rate in 1_000u32..60_000_000,
        sample_rate in 8_000u32..96_000,
        steps in prop::collection::vec((0u64..5_000_000, -500i32..500), 1..64),
    ) {
        let mut blip = BlipBuf::new(clock_rate, sample_rate);
        let mut steps = steps;
        steps.sort_by_key(|s| s.0);
        let mut total = 0i32;
        for (clock, delta) in &steps {
            blip.add_delta(*clock, *delta);
            total += delta;
        }
        let mut last = 0i16;
        /* Steps beyond the look-ahead window are placed at its end, so
         * draining one window plus the kernel reaches the settled level. */
        let horizon = sample_rate as usize / 10 + 100;
        let mut buf = [0i16; 256];
        let mut read = 0usize;
        while read < horizon {
            let n = blip.read_samples(&mut buf);
            read += n;
            last = buf[n - 1];
        }
        prop_assert_eq!(blip.read_instant() as i32, total.clamp(-32768, 32767));
        prop_assert_eq!(last as i32, total);
    }

    /// A step's position in the output tracks its timestamp: it crosses the
    /// midpoint `latency` samples after its (rounded) sample time.
    #[test]
    fn prop_step_position_tracks_time(offset in 0u64..2_000) {
        let mut blip = BlipBuf::new(48_000, 48_000);
        read_n(&mut blip, 500);
        blip.add_delta(500 + offset, 10_000);
        let out = read_n(&mut blip, offset as usize + 64);
        let crossing = out.iter().position(|&s| s >= 5_000).unwrap();
        prop_assert!((crossing as i64 - (offset as usize + LATENCY) as i64).abs() <= 1);
    }
}
