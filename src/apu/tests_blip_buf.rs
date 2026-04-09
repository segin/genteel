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

    // Read 16 samples. The band-limited kernel delays the step by half the kernel size (8 samples)
    // The first ~8 samples should be close to 0, and the remaining samples should be close to 1000
    // Actually, based on empirical behavior, it is [0, 0, 0, 0, 0, 0, 0, 0, 999, 999, 999, 999, 999, 999, 999, 999]
    let mut samples = [0i16; 16];
    let count = blip.read_samples(&mut samples);
    assert_eq!(count, 16);

    for i in 0..8 {
        assert_eq!(samples[i], 0);
    }
    for i in 8..16 {
        assert_eq!(samples[i], 999);
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
    assert_eq!(samples[0], -1);
    assert_eq!(samples[1], 0);
    assert_eq!(samples[2], -7);
    assert_eq!(samples[6], -131);
    assert_eq!(samples[7], 495);
    assert_eq!(samples[8], 1121); // Notice the ringing
    assert_eq!(samples[9], 937);
    assert_eq!(samples[15], 990); // Eventually settles near 1000
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
