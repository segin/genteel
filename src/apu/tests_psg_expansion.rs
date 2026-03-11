use super::psg::Psg;

#[test]
fn test_psg_tone_0_full_cycle() {
    let mut psg = Psg::new();
    // Frequency 10 (0x00A)
    psg.write(0x8A); // Latch ch 0, freq low 0xA
    psg.write(0x00); // Data ch 0, freq high 0x0
                     // Volume 0 (Max)
    psg.write(0x90);

    assert_eq!(psg.tones[0].frequency, 10);
    assert_eq!(psg.tones[0].volume, 0);

    // Initial state: output=false, last_amp=0
    // Manually force output=true to test the decrement/reload cycle
    psg.tones[0].output = true;
    psg.tones[0].last_amp = 4095;
    psg.tones[0].counter = 10;
    assert_eq!(psg.tones[0].last_amp, 4095);

    // Step 1-9: counter 10->9->8->7->6->5->4->3->2->1.
    for _ in 0..9 {
        psg.step_cycles(1);
        assert_eq!(psg.tones[0].last_amp, 4095);
    }

    // Step 10: counter=1 -> 0.
    // In our implementation, when counter reaches 0 it toggles immediately.
    psg.step_cycles(1);
    assert_eq!(psg.tones[0].last_amp, 0);
    assert_eq!(psg.tones[0].counter, 10);
    assert!(!psg.tones[0].output);

    // Step 11-19: counter 10->1
    for _ in 0..9 {
        psg.step_cycles(1);
        assert_eq!(psg.tones[0].last_amp, 0);
    }

    // Step 20: counter=1 -> 0. Toggles back to true.
    psg.step_cycles(1);
    assert_eq!(psg.tones[0].last_amp, 4095);
    assert_eq!(psg.tones[0].counter, 10);
    assert!(psg.tones[0].output);
}

#[test]
fn test_psg_all_tones_mixing() {
    let mut psg = Psg::new();

    // Tone 0: Freq 10, Vol 0 (4095)
    psg.write(0x8A);
    psg.write(0x00);
    psg.write(0x90);
    // Tone 1: Freq 20, Vol 2 (2584)
    psg.write(0xAA);
    psg.write(0x01);
    psg.write(0xB2);
    // Tone 2: Freq 30, Vol 4 (1630)
    psg.write(0xCA);
    psg.write(0x01);
    psg.write(0xD4);

    // Force all high for calculation
    psg.tones[0].output = true;
    psg.tones[0].last_amp = 4095;
    psg.tones[1].output = true;
    psg.tones[1].last_amp = 2584;
    psg.tones[2].output = true;
    psg.tones[2].last_amp = 1630;
    psg.noise.volume = 15;

    let sample = psg.current_sample();
    let expected = 4095 + 2584 + 1630;
    assert_eq!(sample as i32, expected);
}

#[test]
fn test_psg_noise_white_vs_periodic() {
    let mut psg = Psg::new();

    // Periodic noise, rate 0 (N/512)
    psg.write(0xE0); // 1110 0000: ch3, freq, white=0, rate=0
    psg.write(0xF0); // max volume

    let mut periodic_samples = Vec::new();
    for _ in 0..1000 {
        psg.step_cycles(1);
        periodic_samples.push(psg.noise.last_amp);
    }

    // White noise, rate 0
    psg.reset();
    psg.write(0xE4); // 1110 0100: ch3, freq, white=1, rate=0
    psg.write(0xF0);

    let mut white_samples = Vec::new();
    for _ in 0..1000 {
        psg.step_cycles(1);
        white_samples.push(psg.noise.last_amp);
    }

    assert_ne!(
        periodic_samples, white_samples,
        "Periodic and white noise should differ"
    );
}

#[test]
fn test_psg_noise_rate_3_tone_2() {
    let mut psg = Psg::new();

    // Tone 2: Freq 50
    psg.write(0xC2); // Latch ch 2, freq, low 0x2
    psg.write(0x03); // Data ch 2, freq, high 0x3. Result (0x03 << 4) | 0x2 = 0x32 = 50.

    // Noise: rate 3 (use tone 2)
    psg.write(0xE3);
    psg.write(0xF0);

    // Initial counter for noise should eventually reload from tone 2 freq
    psg.noise.counter = 0;
    psg.step_cycles(1);
    assert_eq!(psg.noise.counter, 50);
}
