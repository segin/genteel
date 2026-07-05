use super::ym2612::{Bank, Ym2612};

/* Ladder-effect baseline: 6 channels x (4 << 5) = 768, scaled by the
 * BlastEm-faithful 79/120 output gain (see Ym2612::finish_sample). */
const LADDER_BASELINE: i16 = (768 * 79 / 120) as i16;

fn configure_basic_tone(ym: &mut Ym2612, bank: Bank, offset: u8, algorithm: u8) {
    ym.write_addr(bank, 0xA4 + offset);
    ym.write_data_bank(bank, 0x22);
    ym.write_addr(bank, 0xA0 + offset);
    ym.write_data_bank(bank, 0x55);

    ym.write_addr(bank, 0xB0 + offset);
    ym.write_data_bank(bank, algorithm & 0x07);

    for op_off in [0u8, 4, 8, 12] {
        ym.write_addr(bank, 0x40 + offset + op_off);
        ym.write_data_bank(bank, 0x00);
        ym.write_addr(bank, 0x50 + offset + op_off);
        ym.write_data_bank(bank, 0x1F);
    }

    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0xF0);
}

#[test]
fn test_ym2612_all_channels_enable() {
    let mut ym = Ym2612::new();

    // Enable all 6 channels by setting frequency, volume, attack rate, algorithm, and key-on
    for ch in 0..6 {
        let (bank, offset) = if ch < 3 {
            (Bank::Bank0, ch)
        } else {
            (Bank::Bank1, ch - 3)
        };

        ym.write_addr(bank, 0xA4 + offset as u8);
        ym.write_data_bank(bank, 0x22);
        ym.write_addr(bank, 0xA0 + offset as u8);
        ym.write_data_bank(bank, 0x55);

        ym.write_addr(bank, 0xB0 + offset as u8);
        ym.write_data_bank(bank, 0x07);

        for op_off in [0u8, 4, 8, 12] {
            ym.write_addr(bank, 0x40 + offset as u8 + op_off);
            ym.write_data_bank(bank, 0x00);
            ym.write_addr(bank, 0x50 + offset as u8 + op_off);
            ym.write_data_bank(bank, 0x1F);
        }

        let ch_bits = match ch {
            0..=2 => ch as u8,
            3..=5 => (ch as u8) + 1,
            _ => 7,
        };
        ym.write_addr(Bank::Bank0, 0x28);
        ym.write_data_bank(Bank::Bank0, 0xF0 | ch_bits);
    }

    // Step internal logic
    let mut saw_nonzero = false;
    for _ in 0..1000 {
        ym.step(1);
        if ym.generate_channel_samples().iter().any(|&s| s != 0) {
            saw_nonzero = true;
            break;
        }
    }
    assert!(
        saw_nonzero,
        "Samples should be non-zero when channels are active"
    );
}

#[test]
fn test_ym2612_algorithm_4_has_parallel_carrier_output() {
    let mut ym = Ym2612::new();
    configure_basic_tone(&mut ym, Bank::Bank0, 0, 0x04);

    // Mute operator 4 so only operator 2 can contribute if the routing is correct.
    ym.write_addr(Bank::Bank0, 0x40 + 12);
    ym.write_data_bank(Bank::Bank0, 0x7F);

    let mut saw_nonzero = false;
    for _ in 0..2000 {
        ym.step(1);
        if ym.generate_channel_samples()[0] != 0 {
            saw_nonzero = true;
            break;
        }
    }

    assert!(
        saw_nonzero,
        "Algorithm 4 should still output through operator 2 when operator 4 is muted"
    );
}

#[test]
fn test_ym2612_algorithm_6_has_multiple_carriers() {
    let mut ym = Ym2612::new();
    configure_basic_tone(&mut ym, Bank::Bank0, 0, 0x06);

    // Mute operator 4 so operators 2 and 3 must still be audible.
    ym.write_addr(Bank::Bank0, 0x40 + 12);
    ym.write_data_bank(Bank::Bank0, 0x7F);

    let mut saw_nonzero = false;
    for _ in 0..2000 {
        ym.step(1);
        if ym.generate_channel_samples()[0] != 0 {
            saw_nonzero = true;
            break;
        }
    }

    assert!(
        saw_nonzero,
        "Algorithm 6 should still output through operators 2 and 3 when operator 4 is muted"
    );
}

#[test]
fn test_ym2612_dac_panning() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x2B);
    ym.write_data_bank(Bank::Bank0, 0x80);

    ym.write_addr(Bank::Bank0, 0x2A);
    ym.write_data_bank(Bank::Bank0, 0xFF);

    // Set Pan Left Only: 0x80
    ym.write_addr(Bank::Bank1, 0xB6);
    ym.write_data_bank(Bank::Bank1, 0x80);

    ym.step(144);
    assert!(ym.blip_l.read_instant() > 0, "Left should be positive");
    assert_eq!(
        ym.blip_r.read_instant(),
        LADDER_BASELINE,
        "Right should contain only the YM2612 ladder-effect baseline"
    );

    // Set Pan Right Only: 0x40
    ym.write_addr(Bank::Bank1, 0xB6);
    ym.write_data_bank(Bank::Bank1, 0x40);
    ym.step(144);
    assert_eq!(
        ym.blip_l.read_instant(),
        LADDER_BASELINE,
        "Left should contain only the YM2612 ladder-effect baseline"
    );
    assert!(ym.blip_r.read_instant() > 0, "Right should be positive");
}

#[test]
fn test_ym2612_dac_write_applies_on_next_sample_boundary() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank1, 0xB6);
    ym.write_data_bank(Bank::Bank1, 0x80);

    ym.write_addr(Bank::Bank0, 0x2B);
    ym.write_data_bank(Bank::Bank0, 0x80);

    ym.write_addr(Bank::Bank0, 0x2A);
    ym.write_data_bank(Bank::Bank0, 0xFF);

    assert_eq!(ym.blip_l.read_instant(), 0);
    assert_eq!(ym.blip_r.read_instant(), 0);

    ym.step(143);
    assert_eq!(ym.blip_l.read_instant(), 0);
    assert_eq!(ym.blip_r.read_instant(), 0);

    ym.step(1);
    assert!(ym.blip_l.read_instant() > 0);
    assert_eq!(ym.blip_r.read_instant(), LADDER_BASELINE);
}

#[test]
fn test_ym2612_mid_sample_dac_write_after_channel6_slot_waits_for_next_sample() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank1, 0xB6);
    ym.write_data_bank(Bank::Bank1, 0x80);

    ym.write_addr(Bank::Bank0, 0x2B);
    ym.write_data_bank(Bank::Bank0, 0x80);
    ym.write_addr(Bank::Bank0, 0x2A);
    ym.write_data_bank(Bank::Bank0, 0xFF);

    ym.step(144);
    let previous_left = ym.blip_l.read_instant();
    assert!(previous_left > LADDER_BASELINE);
    assert_eq!(ym.blip_r.read_instant(), LADDER_BASELINE);

    // 21 internal YM cycles (126 M68K cycles) lands just after channel 6 has been clocked.
    ym.step(126);
    ym.write_addr(Bank::Bank0, 0x2A);
    ym.write_data_bank(Bank::Bank0, 0x80);

    // Finish the current sample; output should still reflect the earlier latched DAC value.
    ym.step(18);
    assert_eq!(ym.blip_l.read_instant(), previous_left);
    assert_eq!(ym.blip_r.read_instant(), LADDER_BASELINE);

    // The updated DAC value should not become audible until the next completed sample.
    ym.step(144);
    assert_eq!(ym.blip_l.read_instant(), LADDER_BASELINE);
    assert_eq!(ym.blip_r.read_instant(), LADDER_BASELINE);
}

#[test]
fn test_ym2612_mid_sample_channel6_pan_write_after_slot_waits_for_next_sample() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank1, 0xB6);
    ym.write_data_bank(Bank::Bank1, 0x80);

    ym.write_addr(Bank::Bank0, 0x2B);
    ym.write_data_bank(Bank::Bank0, 0x80);
    ym.write_addr(Bank::Bank0, 0x2A);
    ym.write_data_bank(Bank::Bank0, 0xFF);

    ym.step(144);
    let previous_left = ym.blip_l.read_instant();
    assert!(previous_left > LADDER_BASELINE);
    assert_eq!(ym.blip_r.read_instant(), LADDER_BASELINE);

    // 21 internal YM cycles (126 M68K cycles) lands just after channel 6 has been clocked.
    ym.step(126);
    ym.write_addr(Bank::Bank1, 0xB6);
    ym.write_data_bank(Bank::Bank1, 0x40);

    // Finish the current sample; the pan change should wait for the next sample boundary.
    ym.step(18);
    assert_eq!(ym.blip_l.read_instant(), previous_left);
    assert_eq!(ym.blip_r.read_instant(), LADDER_BASELINE);

    ym.step(144);
    assert_eq!(ym.blip_l.read_instant(), LADDER_BASELINE);
    assert!(ym.blip_r.read_instant() > LADDER_BASELINE);
}

#[test]
fn test_ym2612_timer_ab_simultaneous() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x24);
    ym.write_data_bank(Bank::Bank0, 0xFF);
    ym.write_addr(Bank::Bank0, 0x25);
    ym.write_data_bank(Bank::Bank0, 0x03);

    ym.write_addr(Bank::Bank0, 0x26);
    ym.write_data_bank(Bank::Bank0, 0xFF);

    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x0F);

    // Timer B ticks once every 16 YM samples, so it needs 16x the sample budget.
    for _ in 0..(2304 * 16) {
        ym.step(1);
    }

    assert_eq!(
        ym.read_status() & 0x03,
        0x03,
        "Both timers should have fired"
    );
}
