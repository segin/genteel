use super::blip_buf::BlipBuf;

#[test]
fn test_initialization() {
    let clock_rate = 3579545; // PSG clock rate
    let sample_rate = 44100;
    let blip = BlipBuf::new(clock_rate, sample_rate);

    assert_eq!(blip.read_instant(), 0);
}

#[test]
fn test_clear() {
    let clock_rate = 3579545;
    let sample_rate = 44100;
    let mut blip = BlipBuf::new(clock_rate, sample_rate);

    blip.add_delta(0, 100);
    assert_eq!(blip.read_instant(), 100);

    blip.clear();
    assert_eq!(blip.read_instant(), 0);
}

#[test]
fn test_add_delta_and_read_instant() {
    let clock_rate = 3579545;
    let sample_rate = 44100;
    let mut blip = BlipBuf::new(clock_rate, sample_rate);

    blip.add_delta(0, 500);
    assert_eq!(blip.read_instant(), 500);

    blip.add_delta(100, -200);
    assert_eq!(blip.read_instant(), 300);

    blip.add_delta(200, 40000); // Should clamp
    assert_eq!(blip.read_instant(), 32767);
}

#[test]
fn test_read_samples_kernel_latency() {
    let clock_rate = 44100;
    let sample_rate = 44100;
    let mut blip = BlipBuf::new(clock_rate, sample_rate);

    // Apply a delta at clock 0
    blip.add_delta(0, 1000);

    // Read 16 samples. The band-limited kernel delays the step by half the
    // kernel size (8 samples): the first 8 samples are 0 and the remainder
    // settle at exactly 1000 (the taps sum to exactly the delta).
    let mut samples = [0i16; 16];
    let count = blip.read_samples(&mut samples);
    assert_eq!(count, 16);

    for i in 0..8 {
        assert_eq!(samples[i], 0);
    }
    for i in 8..16 {
        assert_eq!(samples[i], 1000);
    }
}

#[test]
fn test_sinc_filtering_fractional_offset() {
    let clock_rate = 44100;
    let sample_rate = 44100;
    let mut blip = BlipBuf::new(clock_rate, sample_rate);

    // Apply delta at clock 0.5 (fractional)
    // To do this, we need to temporarily lie to the blip buf about the clock rate to cause a fractional absolute sample index
    blip.set_clock_rate(clock_rate * 2);
    blip.add_delta(1, 1000); // 1 source clock / (clock_rate * 2) = 0.5 samples

    let mut samples = [0i16; 16];
    let count = blip.read_samples(&mut samples);
    assert_eq!(count, 16);

    // We expect some sinc filtering behavior, where the transition isn't perfectly aligned
    assert_eq!(samples[0], 0);
    assert_eq!(samples[1], 1);
    assert_eq!(samples[2], -4);
    assert_eq!(samples[6], -126);
    assert_eq!(samples[7], 500);
    assert_eq!(samples[8], 1126); // Notice the ringing
    assert_eq!(samples[9], 942);
    assert_eq!(samples[15], 1000); // Settles at exactly 1000
}

#[test]
fn test_alternating_steps_do_not_drift_the_integrator() {
    /* Regression: per-tap `>> 15` truncation rounded toward -inf, biasing
     * every step slightly negative; alternating +/-delta steps railed the
     * integrated output at -32768 after ~100k steps. With exact tap
     * distribution the band-limited output must track the square wave. */
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
            if chip_i % 24 == 0 {
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
fn test_set_timing_updates_rates_and_clears_state() {
    let mut blip = BlipBuf::new(3579545, 44100);
    blip.add_delta(0, 500);
    assert_eq!(blip.read_instant(), 500);

    blip.set_timing(53203424, 48000);

    assert_eq!(blip.clock_rate(), 53203424);
    assert_eq!(blip.sample_rate(), 48000);
    assert_eq!(blip.read_instant(), 0);

    blip.add_delta(0, 250);
    assert_eq!(blip.read_instant(), 250);
}

#[test]
fn test_set_timing_same_values_is_noop() {
    let mut blip = BlipBuf::new(3579545, 44100);
    blip.add_delta(0, 500);

    blip.set_timing(3579545, 44100);

    assert_eq!(blip.clock_rate(), 3579545);
    assert_eq!(blip.sample_rate(), 44100);
    assert_eq!(blip.read_instant(), 500);
}

#[test]
fn test_long_running_delta_schedule_does_not_drift_out_of_window() {
    let clock_rate = 53_267;
    let sample_rate = 53_267;
    let mut blip = BlipBuf::new(clock_rate, sample_rate);

    let mut sample = [0i16; 1];
    for clock in 0..(sample_rate as u64 * 3) {
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
}
