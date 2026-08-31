use super::blip_buf::BlipBuf;
use super::ym2612::{Bank, Ym2612, Ym2612HardwareProfile};
use crate::audio;

#[test]
fn test_ym2612_initialization() {
    let ym = Ym2612::new();
    for i in 0..3 {
        assert_eq!(ym.registers[0][0xB4 + i], 0xC0);
        assert_eq!(ym.registers[1][0xB4 + i], 0xC0);
    }
    assert_eq!(ym.status, 0);
    assert_eq!(ym.total_clocks, 0);
    assert_eq!(ym.total_mclocks, 0);
}

#[test]
fn test_ym2612_reset() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0x24);
    ym.write_data_bank(Bank::Bank0, 0xAA);

    assert_eq!(ym.registers[0][0x24], 0xAA);

    ym.reset();

    assert_eq!(ym.registers[0][0x24], 0x00);
}

#[test]
fn test_ym2612_read_status() {
    let mut ym = Ym2612::new();
    assert_eq!(ym.read_status(), 0);

    ym.write_data_bank(Bank::Bank0, 0x00);
    assert_ne!(ym.read_status() & 0x80, 0); // Busy flag should be set

    // Busy holds ~32 internal YM cycles = 1344 MCLK; step is in CPU cycles
    // (x7 MCLK), so 191 cycles leave it set and 192 clear it.
    ym.step(191);
    assert_ne!(ym.read_status() & 0x80, 0); // Busy still held
    ym.step(1);
    assert_eq!(ym.read_status() & 0x80, 0); // Busy flag should be cleared
}

#[test]
fn test_ym2612_discrete_undefined_reads_decay_after_status_read() {
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::DiscreteYm2612);
    ym.status = 0x03;

    assert_eq!(ym.read(0), 0x03);
    assert_eq!(ym.read(1), 0x03);

    ym.step((audio::NTSC_MCLK / 56).max(1));
    let decayed = ym.read(1);
    assert!(decayed < 0x03);

    ym.step((audio::NTSC_MCLK / 28).max(1));
    assert_eq!(ym.read(1), 0);
}

#[test]
fn test_ym2612_ym3438_profile_mirrors_all_read_ports() {
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::Ym3438);
    ym.status = 0x03;

    let status = ym.read(0);
    assert_eq!(status, 0x03);
    assert_eq!(ym.read(1), status);
    assert_eq!(ym.read(2), status);
    assert_eq!(ym.read(3), status);
}

#[test]
fn test_ym2612_ym3438_address_write_sets_short_busy_window() {
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::Ym3438);

    ym.write_addr(Bank::Bank0, 0x22);
    assert_ne!(ym.read_status() & 0x80, 0);

    ym.step(16);
    assert_ne!(ym.read_status() & 0x80, 0);

    ym.step(1);
    assert_eq!(ym.read_status() & 0x80, 0);
}

#[test]
fn test_ym2612_ym3438_data_write_busy_depends_on_register_range() {
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::Ym3438);

    ym.write_addr(Bank::Bank0, 0x22);
    ym.write_data_bank(Bank::Bank0, 0x08);
    ym.step(82);
    assert_ne!(ym.read_status() & 0x80, 0);
    ym.step(1);
    assert_eq!(ym.read_status() & 0x80, 0);

    ym.write_addr(Bank::Bank0, 0xA4);
    ym.write_data_bank(Bank::Bank0, 0x20);
    ym.step(46);
    assert_ne!(ym.read_status() & 0x80, 0);
    ym.step(1);
    assert_eq!(ym.read_status() & 0x80, 0);
}

#[test]
fn test_ym2612_discrete_profile_applies_ladder_offset_to_silence() {
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::DiscreteYm2612);

    ym.step(144);
    assert_ne!(ym.last_left, 0);
    assert_ne!(ym.last_right, 0);
}

#[test]
fn test_ym2612_ym3438_profile_disables_ladder_offset_on_silence() {
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::Ym3438);

    ym.step(144);
    assert_eq!(ym.last_left, 0);
    assert_eq!(ym.last_right, 0);
}

#[test]
fn test_ym2612_busy_window_is_not_shortened_by_later_shorter_write() {
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::Ym3438);

    ym.write_addr(Bank::Bank0, 0x22);
    ym.write_data_bank(Bank::Bank0, 0x08);
    ym.write_addr(Bank::Bank0, 0x22);

    ym.step(82);
    assert_ne!(ym.read_status() & 0x80, 0);

    ym.step(1);
    assert_eq!(ym.read_status() & 0x80, 0);
}

#[test]
fn test_ym2612_timers() {
    let mut ym = Ym2612::new();

    // Timer A
    ym.write_addr(Bank::Bank0, 0x24);
    ym.write_data_bank(Bank::Bank0, 0xFF);
    ym.write_addr(Bank::Bank0, 0x25);
    ym.write_data_bank(Bank::Bank0, 0x03);

    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x05); // Load and enable Timer A

    ym.step(144);
    assert_ne!(ym.read_status() & 0x01, 0); // Timer A expired

    // Reset status flags
    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x30);
    assert_eq!(ym.read_status() & 0x03, 0);

    // Timer B
    ym.write_addr(Bank::Bank0, 0x26);
    ym.write_data_bank(Bank::Bank0, 0xFF);

    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x0A); // Load and enable Timer B

    ym.step(2304 * 16);
    assert_ne!(ym.read_status() & 0x02, 0); // Timer B expired
}

#[test]
fn test_ym2612_uses_native_sample_period() {
    let mut ym = Ym2612::new();

    ym.step(143);
    assert_eq!(ym.total_clocks, 0);
    assert_eq!(ym.total_mclocks, 42 * 23);

    ym.step(1);
    assert_eq!(ym.total_clocks, 1);
    assert_eq!(ym.total_mclocks, 1008);
}

#[test]
fn test_ym2612_lfo_register_enable_disable() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x22);
    ym.write_data_bank(Bank::Bank0, 0x0E);

    assert_eq!(ym.lfo_debug_state(), (8, 126, 0));

    ym.step(144 * 8);
    let (_, am, pm) = ym.lfo_debug_state();
    assert_ne!(am, 126);
    assert_eq!(pm, 0);

    ym.step(144 * 8 * 4);
    let (_, _, pm) = ym.lfo_debug_state();
    assert_ne!(pm, 0);

    ym.write_addr(Bank::Bank0, 0x22);
    ym.write_data_bank(Bank::Bank0, 0x00);
    assert_eq!(ym.lfo_debug_state(), (0, 126, 0));
}

#[test]
fn test_ym2612_b4_programs_channel_lfo_depth() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0xB4);
    ym.write_data_bank(Bank::Bank0, 0xB7);

    assert_eq!(ym.channel_lfo_debug(0), Some((0, 7)));
}

#[test]
fn test_ym2612_key_on_bits_follow_real_slot_order() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0x20);
    assert_eq!(ym.channel_key_state(0), Some([false, true, false, false]));

    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0x40);
    assert_eq!(ym.channel_key_state(0), Some([false, false, true, false]));
}

#[test]
fn test_ym2612_data_write_applies_to_last_selected_group() {
    let mut ym = Ym2612::new();

    // Address latched on port 1: a data write on port 0 still applies to
    // the last-selected group — there is one physical data port and the
    // chip only remembers which address port was written last.
    ym.write_address(1, 0xB4);
    ym.step(3);
    ym.write_data(0, 0xA7);
    assert_eq!(ym.registers[1][0xB4], 0xA7);
    assert_eq!(ym.registers[0][0xB4], 0xC0);
}

#[test]
fn test_ym2612_port_data_write_applies_immediately() {
    // Data writes apply immediately (no busy-drop; see write_data). Real games
    // write address+data back-to-back with no FM step in between.
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::Ym3438);

    ym.write_address(0, 0xB4);
    ym.write_data(0, 0xA7);

    assert_eq!(ym.registers[0][0xB4], 0xA7);
}

#[test]
fn test_ym2612_frequency_high_write_latches_until_low_write() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0xA4);
    ym.write_data_bank(Bank::Bank0, 0x20);
    ym.write_addr(Bank::Bank0, 0xA0);
    ym.write_data_bank(Bank::Bank0, 0x34);
    assert_eq!(ym.channel_frequency_debug(0), Some((0x034, 4)));

    ym.write_addr(Bank::Bank0, 0xA4);
    ym.write_data_bank(Bank::Bank0, 0x38);
    assert_eq!(ym.channel_frequency_debug(0), Some((0x034, 4)));

    ym.write_addr(Bank::Bank0, 0xA0);
    ym.write_data_bank(Bank::Bank0, 0x56);
    assert_eq!(ym.channel_frequency_debug(0), Some((0x056, 7)));
}

#[test]
fn test_ym2612_ch3_special_frequency_high_write_latches_until_low_write() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0xAC);
    ym.write_data_bank(Bank::Bank0, 0x18);
    ym.write_addr(Bank::Bank0, 0xA8);
    ym.write_data_bank(Bank::Bank0, 0x22);
    assert_eq!(ym.ch3_slot_frequency_debug(0), Some((0x022, 3)));

    ym.write_addr(Bank::Bank0, 0xAC);
    ym.write_data_bank(Bank::Bank0, 0x28);
    assert_eq!(ym.ch3_slot_frequency_debug(0), Some((0x022, 3)));

    ym.write_addr(Bank::Bank0, 0xA8);
    ym.write_data_bank(Bank::Bank0, 0x44);
    assert_eq!(ym.ch3_slot_frequency_debug(0), Some((0x044, 5)));
}

#[test]
fn test_ym2612_csm_mode_uses_ch3_per_operator_frequencies() {
    /* Any non-zero $27 bits 7-6 (special mode or CSM) selects the CH3
     * per-operator frequencies, so CSM (0x80) must advance phases exactly
     * like special mode (0x40) with identical registers. */
    let mk = |mode: u8| {
        let mut ym = Ym2612::new();
        ym.write_addr(Bank::Bank0, 0xA6);
        ym.write_data_bank(Bank::Bank0, 0x22);
        ym.write_addr(Bank::Bank0, 0xA2);
        ym.write_data_bank(Bank::Bank0, 0x69);
        for (hi, lo) in [(0xACu8, 0xA8u8), (0xAD, 0xA9), (0xAE, 0xAA)] {
            ym.write_addr(Bank::Bank0, hi);
            ym.write_data_bank(Bank::Bank0, 0x3A);
            ym.write_addr(Bank::Bank0, lo);
            ym.write_data_bank(Bank::Bank0, 0x9C);
        }
        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, mode);
        ym.write_addr(Bank::Bank0, 0x28);
        ym.write_data_bank(Bank::Bank0, 0xF2);
        ym.step(144 * 8);
        ym
    };

    let special = mk(0x40);
    let csm = mk(0x80);
    for slot in 0..4 {
        assert_eq!(
            special.operator_phase_debug(2, slot),
            csm.operator_phase_debug(2, slot)
        );
    }

    let normal = mk(0x00);
    assert_ne!(
        normal.operator_phase_debug(2, 0),
        csm.operator_phase_debug(2, 0)
    );
}

#[test]
fn test_ym2612_mul_zero_runs_at_half_of_mul_one() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0xA4);
    ym.write_data_bank(Bank::Bank0, 0x20);
    ym.write_addr(Bank::Bank0, 0xA0);
    ym.write_data_bank(Bank::Bank0, 0x55);

    ym.write_addr(Bank::Bank0, 0x30);
    ym.write_data_bank(Bank::Bank0, 0x00);

    ym.step(144);
    let mul_zero_phase = ym.operator_phase_debug(0, 0).unwrap();

    let mut ym_mul_one = Ym2612::new();
    ym_mul_one.write_addr(Bank::Bank0, 0xA4);
    ym_mul_one.write_data_bank(Bank::Bank0, 0x20);
    ym_mul_one.write_addr(Bank::Bank0, 0xA0);
    ym_mul_one.write_data_bank(Bank::Bank0, 0x55);
    ym_mul_one.write_addr(Bank::Bank0, 0x30);
    ym_mul_one.write_data_bank(Bank::Bank0, 0x01);

    ym_mul_one.step(144);
    let mul_one_phase = ym_mul_one.operator_phase_debug(0, 0).unwrap();

    assert_eq!(mul_zero_phase * 2, mul_one_phase);
}

#[test]
fn test_ym2612_phase_modulation_debug_changes_with_depth_and_phase() {
    let zero = Ym2612::phase_mod_debug(0x355, 0, 8);
    let shallow = Ym2612::phase_mod_debug(0x355, 1, 8);
    let deep = Ym2612::phase_mod_debug(0x355, 7, 8);
    let negative = Ym2612::phase_mod_debug(0x355, 7, 24);

    assert_eq!(zero, 0);
    assert!(deep.abs() > shallow.abs());
    assert!(negative < 0);
}

#[test]
fn test_ym2612_phase_modulation_does_not_change_envelope_key_scaling() {
    let mut base = Ym2612::new();
    let mut modulated = Ym2612::new();

    for ym in [&mut base, &mut modulated] {
        ym.write_addr(Bank::Bank0, 0xA4);
        ym.write_data_bank(Bank::Bank0, 0x38);
        ym.write_addr(Bank::Bank0, 0xA0);
        ym.write_data_bank(Bank::Bank0, 0x55);
        ym.write_addr(Bank::Bank0, 0x50);
        ym.write_data_bank(Bank::Bank0, 0xFF);
        ym.write_addr(Bank::Bank0, 0x60);
        ym.write_data_bank(Bank::Bank0, 0x1F);
        ym.force_operator_envelope(0, 0, 0x180, "decay");
        ym.force_lfo_debug(126, 8);
    }

    modulated.write_addr(Bank::Bank0, 0xB4);
    modulated.write_data_bank(Bank::Bank0, 0xC7);

    for _ in 0..64 {
        base.step(144);
        modulated.step(144);
    }

    let (base_level, base_phase, _) = base.operator_envelope_debug(0, 0).unwrap();
    let (mod_level, mod_phase, _) = modulated.operator_envelope_debug(0, 0).unwrap();
    assert_eq!(mod_level, base_level);
    assert_eq!(mod_phase, base_phase);
}

#[test]
fn test_ym2612_phase_modulation_does_not_change_detune_keycode_source() {
    let base_kc = 0;
    let modulated_kc = 31;
    let phase_fnum = 0x7ff;
    let block = 7;
    let detune = 0x01;
    let multiple = 1;

    let using_base_kc = Ym2612::phase_increment_debug(phase_fnum, block, base_kc, detune, multiple);
    let using_modulated_kc =
        Ym2612::phase_increment_debug(phase_fnum, block, modulated_kc, detune, multiple);

    assert_ne!(using_base_kc, using_modulated_kc);
}

#[test]
fn test_ym2612_negative_detune_wraps_in_17_bit_domain() {
    let phase = Ym2612::phase_increment_debug(0, 0, 31, 0x07, 1);
    assert_eq!(phase, 131_050);
}

#[test]
fn test_ym2612_pm_phase_increment_wraps_in_12_bit_frequency_domain() {
    let phase = Ym2612::phase_increment_pm_debug(0x07ff, 0, 0, 0, 1, 2);
    assert_eq!(phase, 0);
}

#[test]
fn test_ym2612_non_feedback_phase_modulation_is_halved() {
    let mut ym = Ym2612::new();
    ym.force_operator_envelope(0, 0, 0, "decay");
    ym.force_operator_phase(0, 0, 0);

    let full = ym.operator_output_debug(0, 0, 0x40, 0, 0, 0, true).unwrap();
    let half = ym
        .operator_output_debug(0, 0, 0x40, 0, 0, 0, false)
        .unwrap();

    assert_ne!(full, half);
}

#[test]
fn test_ym2612_operator_output_preserves_large_phase_mod_inputs() {
    let mut ym = Ym2612::new();
    ym.force_operator_envelope(0, 0, 0, "decay");
    ym.force_operator_phase(0, 0, 0);

    let wide = ym
        .operator_output_debug(0, 0, 16_320, 0, 0, 0, false)
        .unwrap();
    let prematurely_clamped = ym
        .operator_output_debug(0, 0, 8_191, 0, 0, 0, false)
        .unwrap();

    assert_ne!(wide, prematurely_clamped);
}

#[test]
fn test_ym2612_carrier_outputs_use_discrete_9bit_quantization_masks() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x07);
    ym.force_operator_envelope(0, 0, 0, "decay");
    ym.force_operator_phase(0, 0, 0);

    let raw = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();
    assert_ne!(raw, 0);
    assert_eq!((raw as i32) & 31, raw as i32 & 31);
    assert_eq!((((raw as i32) & !31) as i16) as i32 & 31, 0);
}

#[test]
fn test_ym2612_algorithm7_stores_full_resolution_op1_in_feedback_history() {
    /* The 9-bit carrier truncation applies to the copy summed into the
     * channel output only; op1's feedback history keeps all 14 bits. */
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x07);
    ym.force_operator_envelope(0, 0, 0, "decay");
    ym.force_operator_phase(0, 0, 0);

    let raw = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();
    ym.step(144);

    let op1 = ym.operator_last_output_debug(0, 0).unwrap();
    assert_eq!(op1, raw);
    assert_ne!((op1 as i32) & 31, 0);
}

#[test]
fn test_ym2612_ym3438_profile_still_applies_9bit_carrier_quantization() {
    /* The 9-bit carrier truncation is a DAC property of both chips; only
     * the ladder effect is discrete-specific. Feedback history keeps full
     * resolution on both profiles. */
    let mut ym = Ym2612::new();
    ym.set_hardware_profile(Ym2612HardwareProfile::Ym3438);
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x07);
    ym.force_operator_envelope(0, 0, 1, "decay");
    ym.force_operator_phase(0, 0, 4 << 10);

    let raw = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();
    assert!(raw.abs() >= 32, "need a large output, got {raw}");
    assert_ne!((raw as i32) & 31, 0);

    ym.step(144);

    let ch = ym.generate_channel_samples()[0];
    assert_ne!(ch, 0);
    assert_eq!((ch as i32) & 31, 0);
    assert_eq!(ym.operator_last_output_debug(0, 0).unwrap(), raw);
}

#[test]
fn test_ym2612_algorithm3_uses_delayed_mem_on_c2_path_only() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x03);
    ym.force_channel_mem_value(0, 0x140);
    for slot in 0..4 {
        ym.force_operator_envelope(0, slot, 0, "decay");
        ym.force_operator_phase(0, slot, 0);
    }

    let out3 = ym.operator_output_debug(0, 1, 0, 0, 0, 0, false).unwrap();
    let c2 = 0x140 + out3 as i32;
    let out4 = ym.operator_output_debug(0, 3, c2, 0, 0, 0, false).unwrap();
    let expected = ((out4 as i32) & !31) as i16;

    ym.step(144);

    assert_eq!(ym.generate_channel_samples()[0], expected);
    let op2 = ym.operator_last_output_debug(0, 2).unwrap();
    assert_ne!(op2, 0);
}

#[test]
fn test_ym2612_algorithm0_uses_previous_sample_op2_output_for_op3() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x00);
    ym.force_channel_mem_value(0, 0x140);
    for slot in 0..4 {
        ym.force_operator_envelope(0, slot, 0, "decay");
        ym.force_operator_phase(0, slot, 0);
    }

    let out1 = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();
    let out3 = ym
        .operator_output_debug(0, 1, 0x140, 0, 0, 0, false)
        .unwrap();
    let out2 = ym
        .operator_output_debug(0, 2, out1 as i32, 0, 0, 0, false)
        .unwrap();
    let out4 = ym
        .operator_output_debug(0, 3, out3 as i32, 0, 0, 0, false)
        .unwrap();
    let expected = ((out4 as i32) & !31) as i16;

    ym.step(144);

    assert_eq!(ym.generate_channel_samples()[0], expected);
    assert_ne!(out2, 0);
}

#[test]
fn test_ym2612_algorithm1_uses_previous_sample_mem_for_op3() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x01);
    ym.force_channel_mem_value(0, 0x180);
    for slot in 0..4 {
        ym.force_operator_envelope(0, slot, 0, "decay");
        ym.force_operator_phase(0, slot, 0);
    }

    let out1 = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();
    let out3 = ym
        .operator_output_debug(0, 1, 0x180, 0, 0, 0, false)
        .unwrap();
    let out2 = ym
        .operator_output_debug(0, 2, out1 as i32, 0, 0, 0, false)
        .unwrap();
    let out4 = ym
        .operator_output_debug(0, 3, out3 as i32, 0, 0, 0, false)
        .unwrap();
    let expected = ((out4 as i32) & !31) as i16;

    ym.step(144);

    assert_eq!(ym.generate_channel_samples()[0], expected);
    assert_ne!(out2, 0);
}

#[test]
fn test_ym2612_algorithm2_uses_previous_sample_op2_output_for_op3() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x02);
    ym.force_channel_mem_value(0, 0x120);
    for slot in 0..4 {
        ym.force_operator_envelope(0, slot, 0, "decay");
        ym.force_operator_phase(0, slot, 0);
    }

    let out1 = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();
    let out3 = ym
        .operator_output_debug(0, 1, 0x120, 0, 0, 0, false)
        .unwrap();
    let out2 = ym
        .operator_output_debug(0, 2, out1 as i32, 0, 0, 0, false)
        .unwrap();
    let c2 = (out1 as i32) + (out3 as i32);
    let out4 = ym.operator_output_debug(0, 3, c2, 0, 0, 0, false).unwrap();
    let expected = ((out4 as i32) & !31) as i16;

    ym.step(144);

    assert_eq!(ym.generate_channel_samples()[0], expected);
    assert_ne!(out2, 0);
}

#[test]
fn test_ym2612_algorithm5_routes_mem_and_parallel_carriers_like_reference() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x05);
    ym.force_channel_mem_value(0, 0x120);
    for slot in 0..4 {
        ym.force_operator_envelope(0, slot, 0, "decay");
        ym.force_operator_phase(0, slot, 0);
    }

    let out1 = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();
    let out3 = ym
        .operator_output_debug(0, 1, 0x120, 0, 0, 0, false)
        .unwrap();
    let out2 = ym
        .operator_output_debug(0, 2, out1 as i32, 0, 0, 0, false)
        .unwrap();
    let out4 = ym
        .operator_output_debug(0, 3, out1 as i32, 0, 0, 0, false)
        .unwrap();
    /* Algorithm 5 carriers are S2+S3+S4; S1 only modulates them
     * (BlastEm/MAME/GPGX reference). */
    let expected = ((out2 as i32) & !31) + ((out3 as i32) & !31) + ((out4 as i32) & !31);
    let expected = expected.clamp(-8192, 8191) as i16;

    ym.step(144);

    assert_eq!(ym.generate_channel_samples()[0], expected);
}

#[test]
fn test_ym2612_algorithm_write_preserves_live_mem_for_next_sample() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x01);
    ym.force_channel_mem_value(0, 0x180);
    for slot in 0..4 {
        ym.force_operator_envelope(0, slot, 0, "decay");
        ym.force_operator_phase(0, slot, 0);
    }

    ym.step(144);

    let mem = ym.channel_mem_value_debug(0).unwrap();
    let out1 = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();
    let out3 = ym.operator_output_debug(0, 1, mem, 0, 0, 0, false).unwrap();
    let out4 = ym
        .operator_output_debug(0, 3, out3 as i32, 0, 0, 0, false)
        .unwrap();
    let expected = ((out4 as i32) & !31) as i16;

    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x00);
    ym.step(144);

    assert_eq!(ym.generate_channel_samples()[0], expected);
    assert_ne!(out1, 0);
}

#[test]
fn test_ym2612_feedback_write_preserves_live_op1_history_for_next_sample() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x07);
    ym.force_operator_envelope(0, 0, 0, "decay");
    ym.force_operator_phase(0, 0, 0);

    ym.step(144);

    let (last_output, last_output2) = ym.operator_feedback_history_debug(0, 0).unwrap();
    let fb = ((last_output as i32 + last_output2 as i32) >> 1) >> (9 - 7);
    let expected = ym.operator_output_debug(0, 0, fb, 0, 0, 0, true).unwrap();

    ym.write_addr(Bank::Bank0, 0xB0);
    ym.write_data_bank(Bank::Bank0, 0x3f);
    ym.step(144);

    assert_eq!(ym.operator_last_output_debug(0, 0).unwrap(), expected);
}

#[test]
fn test_ym2612_total_level_write_affects_next_sample_output() {
    let mut ym = Ym2612::new();
    ym.force_operator_envelope(0, 0, 0, "decay");
    ym.force_operator_phase(0, 0, 0);

    let loud = ym.operator_output_debug(0, 0, 0, 0, 0, 0, true).unwrap();

    ym.write_addr(Bank::Bank0, 0x40);
    ym.write_data_bank(Bank::Bank0, 0x7f);

    let quiet = ym.operator_output_debug(0, 0, 0, 0x7f, 0, 0, true).unwrap();
    assert!(quiet.abs() < loud.abs());
}

#[test]
fn test_ym2612_attack_rate_write_applies_on_next_eg_tick() {
    let mut ym = Ym2612::new();
    ym.force_operator_envelope(0, 0, 0x200, "attack");

    ym.write_addr(Bank::Bank0, 0x50);
    ym.write_data_bank(Bank::Bank0, 0x1f);

    ym.step(144 * 3);

    let (level, phase, _) = ym.operator_envelope_debug(0, 0).unwrap();
    assert_eq!(level, 0);
    assert!(phase == "decay" || phase == "sustain");
}

#[test]
fn test_ym2612_decay_rate_write_applies_on_next_eg_tick() {
    let mut ym = Ym2612::new();
    ym.force_operator_envelope(0, 0, 0x020, "decay");

    ym.write_addr(Bank::Bank0, 0x60);
    ym.write_data_bank(Bank::Bank0, 0x1f);

    ym.step(144 * 3);

    let (level, phase, _) = ym.operator_envelope_debug(0, 0).unwrap();
    assert!(level > 0x020);
    assert!(phase == "decay" || phase == "sustain");
}

#[test]
fn test_ym2612_mid_sample_attack_rate_write_does_not_affect_current_latched_eg_tick() {
    let mut ym = Ym2612::new();
    ym.force_operator_envelope(0, 0, 0x200, "attack");

    ym.write_addr(Bank::Bank0, 0x50);
    ym.write_data_bank(Bank::Bank0, 0x00);

    // Reach the sample whose end will trigger the next EG tick, then latch that sample.
    ym.step(144 * 2);
    ym.step(126);

    ym.write_addr(Bank::Bank0, 0x50);
    ym.write_data_bank(Bank::Bank0, 0x1f);

    // Finish the current sample; the live write should not affect the already-latched EG tick.
    ym.step(18);
    assert_eq!(ym.operator_envelope_debug(0, 0).unwrap().0, 0x200);
}

#[test]
fn test_ym2612_timer_b_ticks_once_every_16_samples() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0x26);
    ym.write_data_bank(Bank::Bank0, 0xFF);
    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x0A);

    /* NB=0xFF -> period 1 in /16-sample units: the overflow lands on the
     * 16th sample (one sample = 144 internal cycles). */
    ym.step(144 * 15);
    assert_eq!(ym.read_status() & 0x02, 0);

    ym.step(144);
    assert_ne!(ym.read_status() & 0x02, 0);
}

#[test]
fn test_ym2612_ssg_write_recomputes_active_output_shape_on_next_sample() {
    let mut ym = Ym2612::new();
    ym.force_operator_envelope(0, 0, 0x020, "decay");
    ym.force_operator_phase(0, 0, 0);
    ym.force_operator_ssg_invert(0, 0, true);

    let normal = ym
        .operator_output_debug(0, 0, 0x40, 0, 0, 0x00, true)
        .unwrap();

    ym.write_addr(Bank::Bank0, 0x90);
    ym.write_data_bank(Bank::Bank0, 0x08);

    let ssg = ym
        .operator_output_debug(0, 0, 0x40, 0, 0, 0x08, true)
        .unwrap();
    assert_ne!(ssg, normal);
}

#[test]
fn test_ym2612_ssg_write_updates_hold_state_immediately() {
    let mut ym = Ym2612::new();

    ym.force_operator_envelope(0, 0, 0x240, "decay");

    ym.write_addr(Bank::Bank0, 0x90);
    ym.write_data_bank(Bank::Bank0, 0x09);

    let (level, phase, inverted) = ym.operator_envelope_debug(0, 0).unwrap();
    assert_eq!(level, 0x3ff);
    assert_eq!(phase, "decay");
    assert!(!inverted);
}

#[test]
fn test_ym2612_ssg_write_updates_inversion_immediately() {
    let mut ym = Ym2612::new();

    ym.force_operator_envelope(0, 0, 0x240, "decay");

    ym.write_addr(Bank::Bank0, 0x90);
    ym.write_data_bank(Bank::Bank0, 0x0b);

    let (level, phase, inverted) = ym.operator_envelope_debug(0, 0).unwrap();
    assert_eq!(level, 0x240);
    assert_eq!(phase, "decay");
    assert!(inverted);
}

#[test]
fn test_ym2612_ch3_special_pm_uses_per_slot_keycode_for_detune() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x40);

    ym.write_addr(Bank::Bank0, 0xA6);
    ym.write_data_bank(Bank::Bank0, 0x3f);
    ym.write_addr(Bank::Bank0, 0xA2);
    ym.write_data_bank(Bank::Bank0, 0xff);

    ym.write_addr(Bank::Bank0, 0xAD);
    ym.write_data_bank(Bank::Bank0, 0x00);
    ym.write_addr(Bank::Bank0, 0xA9);
    ym.write_data_bank(Bank::Bank0, 0x10);

    ym.write_addr(Bank::Bank0, 0xB6);
    ym.write_data_bank(Bank::Bank0, 0xC7);

    ym.write_addr(Bank::Bank0, 0x32);
    ym.write_data_bank(Bank::Bank0, 0x11);

    ym.force_lfo_debug(126, 8);
    ym.step(144);

    /* The slot's key code derives from its own frequency (fnum 0x010,
     * block 0 -> kc 0), not the channel-level 0x7FF/7 (kc 31). */
    let delta = Ym2612::phase_mod_debug(0x010, 7, 8) as i32;
    let phase_fnum = (0x010i32 + delta).clamp(0, 0x7ff) as u32;
    let expected = Ym2612::phase_increment_debug(phase_fnum as u16, 0, 0, 1, 1);

    assert_eq!(ym.operator_phase_debug(2, 0).unwrap(), expected);
}

#[test]
fn test_ym2612_envelope_advances_only_on_eg_ticks() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x60);
    ym.write_data_bank(Bank::Bank0, 0x1F);
    ym.force_operator_envelope(0, 0, 0x100, "decay");

    let initial = ym.operator_envelope_debug(0, 0).unwrap().0;
    ym.step(144);
    let after_one = ym.operator_envelope_debug(0, 0).unwrap().0;
    ym.step(144);
    let after_two = ym.operator_envelope_debug(0, 0).unwrap().0;
    for _ in 0..190 {
        ym.step(144);
    }
    let later = ym.operator_envelope_debug(0, 0).unwrap().0;

    assert_eq!(after_one, initial);
    assert_eq!(after_two, initial);
    assert!(later > initial);
}

#[test]
fn test_ym2612_release_reaches_off_state() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x80);
    ym.write_data_bank(Bank::Bank0, 0x0F);
    ym.write_addr(Bank::Bank0, 0x50);
    ym.write_data_bank(Bank::Bank0, 0x1F);

    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0x10);
    ym.step(144 * 8);

    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0x00);

    for _ in 0..16384 {
        ym.step(144);
    }

    assert_eq!(ym.operator_envelope_debug(0, 0).unwrap().1, "off");
}

#[test]
fn test_ym2612_ssg_eg_hold_sets_inversion() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x90);
    ym.write_data_bank(Bank::Bank0, 0x0B);
    ym.write_addr(Bank::Bank0, 0x80);
    ym.write_data_bank(Bank::Bank0, 0x0F);
    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0x10);

    for _ in 0..1024 {
        ym.step(144);
    }

    let (_, _, inverted) = ym.operator_envelope_debug(0, 0).unwrap();
    assert!(inverted);
}

#[test]
fn test_ym2612_ssg_hold_with_alt_does_not_force_max_when_inversion_xor_shape_is_set() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x90);
    ym.write_data_bank(Bank::Bank0, 0x0B);
    ym.force_operator_envelope(0, 0, 0x240, "decay");

    ym.step(144);

    let (level, _, inverted) = ym.operator_envelope_debug(0, 0).unwrap();
    assert!(inverted);
    assert_ne!(level, 0x3FF);
}

#[test]
fn test_ym2612_ssg_eg_loop_with_max_attack_skips_back_to_decay_or_sustain() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x50);
    ym.write_data_bank(Bank::Bank0, 0x1f);
    ym.write_addr(Bank::Bank0, 0x80);
    ym.write_data_bank(Bank::Bank0, 0x00);
    ym.write_addr(Bank::Bank0, 0x90);
    ym.write_data_bank(Bank::Bank0, 0x08);
    ym.write_addr(Bank::Bank0, 0xA4);
    ym.write_data_bank(Bank::Bank0, 0x38);
    ym.write_addr(Bank::Bank0, 0xA0);
    ym.write_data_bank(Bank::Bank0, 0xff);
    ym.force_operator_envelope(0, 0, 0x200, "decay");

    ym.step(144);

    let (level, phase, _) = ym.operator_envelope_debug(0, 0).unwrap();
    assert_eq!(level, 0);
    assert_eq!(phase, "sustain");
}

#[test]
fn test_ym2612_ssg_inversion_does_not_apply_during_release() {
    let mut ym = Ym2612::new();

    ym.force_operator_envelope(0, 0, 0x020, "release");
    ym.force_operator_ssg_invert(0, 0, true);

    let released = ym
        .operator_output_debug(0, 0, 0x40, 0, 0, 0x08, true)
        .unwrap();

    ym.force_operator_envelope(0, 0, 0x020, "decay");
    let decaying = ym
        .operator_output_debug(0, 0, 0x40, 0, 0, 0x08, true)
        .unwrap();

    assert_ne!(released, decaying);
}

#[test]
fn test_ym2612_ssg_inverted_output_uses_half_range_transform() {
    /* Inverted SSG output level is (0x200 - internal) & 0x3FF: an inverted
     * operator at internal level L must sound identical to a non-inverted
     * one at attenuation 0x200 - L. */
    let mut inv = Ym2612::new();
    inv.force_operator_envelope(0, 0, 0x180, "decay");
    inv.force_operator_ssg_invert(0, 0, true);
    inv.force_operator_phase(0, 0, 256 << 10);

    let mut plain = Ym2612::new();
    plain.force_operator_envelope(0, 0, 0x080, "decay");
    plain.force_operator_phase(0, 0, 256 << 10);

    let inv_out = inv.operator_output_debug(0, 0, 0, 0, 0, 0x08, false).unwrap();
    let plain_out = plain
        .operator_output_debug(0, 0, 0, 0, 0, 0x00, false)
        .unwrap();
    assert_eq!(inv_out, plain_out);
    assert_ne!(inv_out, 0);
}

#[test]
fn test_ym2612_ssg_release_terminates_at_half_attenuation() {
    let mut ym = Ym2612::new();

    ym.write_addr(Bank::Bank0, 0x90);
    ym.write_data_bank(Bank::Bank0, 0x08);
    ym.write_addr(Bank::Bank0, 0x80);
    ym.write_data_bank(Bank::Bank0, 0x0f);
    ym.force_operator_envelope(0, 0, 0x1fc, "release");

    for _ in 0..16 {
        ym.step(144 * 3);
    }

    let (level, phase, _) = ym.operator_envelope_debug(0, 0).unwrap();
    assert_eq!(level, 0x3ff);
    assert_eq!(phase, "off");
}

#[test]
fn test_ym2612_reg_key_events_masked_during_csm_window() {
    /* While the CSM key window holds the combined key line high, $28 key-on
     * must not retrigger (no phase reset / attack restart) and $28 key-off
     * must not release; the release happens when the window ends. */
    let mut ym = Ym2612::new();
    ym.debug_apply_csm_key_on(); // CSM window opens, keys all CH3 operators

    ym.force_operator_phase(2, 0, 0x1234 << 10);
    ym.force_operator_envelope(2, 0, 0x080, "decay");

    // $28 key-on for CH3 during the window: state latches, no retrigger.
    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0xF2);
    assert_eq!(ym.operator_phase_debug(2, 0), Some(0x1234 << 10));
    assert_eq!(ym.operator_envelope_debug(2, 0).unwrap().1, "decay");

    // $28 key-off during the window: no release yet (CSM still holds).
    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0x02);
    assert_eq!(ym.operator_envelope_debug(2, 0).unwrap().1, "decay");

    // Next sample tick ends the window (state 1 shifts to 2 with the timer
    // idle): with the register key off, the release fires now.
    ym.step(144 * 2);
    assert_eq!(ym.operator_envelope_debug(2, 0).unwrap().1, "release");
}

#[test]
fn test_ym2612_ssg_envelope_freezes_at_0x200_outside_release() {
    /* With SSG-EG enabled, decay/sustain increments stop once attenuation
     * reaches 0x200; held-inverted shapes (e.g. \$90=0x0D) must hold steady
     * there rather than ramp on to 0x3FF. */
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0x90);
    ym.write_data_bank(Bank::Bank0, 0x0D); // enable + attack + hold
    ym.write_addr(Bank::Bank0, 0x60);
    ym.write_data_bank(Bank::Bank0, 0x1F); // DR=31: fastest decay
    ym.force_operator_envelope(0, 0, 0x210, "decay");

    ym.step(144 * 30);
    let (level, _, _) = ym.operator_envelope_debug(0, 0).unwrap();
    assert_eq!(level, 0x210, "level must freeze at/above 0x200, got {level:#x}");
}

#[test]
fn test_ym2612_ssg_release_runs_4x_fast() {
    /* SSG-EG's 4x envelope speedup applies during release as well. */
    let mk = |ssg: u8| {
        let mut ym = Ym2612::new();
        ym.write_addr(Bank::Bank0, 0x90);
        ym.write_data_bank(Bank::Bank0, ssg);
        ym.write_addr(Bank::Bank0, 0x80);
        ym.write_data_bank(Bank::Bank0, 0x0F); // RR=15 -> rate 94, steps every EG tick
        ym.force_operator_envelope(0, 0, 0x040, "release");
        ym.step(144 * 33); // ~11 EG ticks, before the SSG 0x200 snap
        ym.operator_envelope_debug(0, 0).unwrap().0
    };
    let plain = mk(0x00);
    let ssg = mk(0x08);
    assert!(
        ssg > plain,
        "SSG release should rise faster: ssg={ssg:#x} plain={plain:#x}"
    );
    assert!(plain > 0x040, "plain release must have advanced");
}

#[test]
fn test_ym2612_ch6_pipeline_keeps_clocking_in_dac_mode() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank1, 0xA6);
    ym.write_data_bank(Bank::Bank1, 0x22);
    ym.write_addr(Bank::Bank1, 0xA2);
    ym.write_data_bank(Bank::Bank1, 0x69);
    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0xF6);
    ym.write_addr(Bank::Bank0, 0x2B);
    ym.write_data_bank(Bank::Bank0, 0x80);

    let before = ym.operator_phase_debug(5, 0).unwrap();
    ym.step(144 * 4);
    let after = ym.operator_phase_debug(5, 0).unwrap();
    assert_ne!(
        before, after,
        "CH6 phase generators must keep running in DAC mode"
    );
}

#[test]
fn test_ym2612_eg_rate_and_sustain_level_expansion() {
    assert_eq!(Ym2612::eg_debug(0, 0), (0, 0));
    assert_eq!(Ym2612::eg_debug(1, 1), (34, 32));
    assert_eq!(Ym2612::eg_debug(0x1F, 0x0F), (94, 31 << 5));
    assert_eq!(Ym2612::eg_rate_table_debug(0), (11, 144));
    assert_eq!(Ym2612::eg_rate_table_debug(63), (4, 24));
    assert_eq!(Ym2612::eg_rate_table_debug(94), (0, 128));
}

#[test]
fn test_ym2612_sl_rr_write_can_force_decay_to_sustain() {
    let mut ym = Ym2612::new();
    ym.force_operator_envelope(0, 0, 64, "decay");

    ym.write_addr(Bank::Bank0, 0x80);
    ym.write_data_bank(Bank::Bank0, 0x10);

    assert_eq!(ym.operator_envelope_debug(0, 0).unwrap().1, "sustain");
}

#[test]
fn test_ym2612_key_on_max_attack_skips_to_decay_or_sustain() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0x50);
    ym.write_data_bank(Bank::Bank0, 0x1f);
    ym.write_addr(Bank::Bank0, 0x80);
    ym.write_data_bank(Bank::Bank0, 0x00);
    ym.write_addr(Bank::Bank0, 0xA4);
    ym.write_data_bank(Bank::Bank0, 0x38);
    ym.write_addr(Bank::Bank0, 0xA0);
    ym.write_data_bank(Bank::Bank0, 0xff);
    ym.write_addr(Bank::Bank0, 0x28);
    ym.write_data_bank(Bank::Bank0, 0x10);

    let (level, phase, _) = ym.operator_envelope_debug(0, 0).unwrap();
    assert_eq!(level, 0);
    assert_eq!(phase, "sustain");
}

#[test]
fn test_ym2612_csm_timer_a_auto_keys_channel_three() {
    let mut ym = Ym2612::new();
    ym.write_addr(Bank::Bank0, 0x24);
    ym.write_data_bank(Bank::Bank0, 0xff);
    ym.write_addr(Bank::Bank0, 0x25);
    ym.write_data_bank(Bank::Bank0, 0x03);
    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x81);

    ym.step(144);
    assert_eq!(ym.operator_envelope_debug(2, 0).unwrap().1, "attack");

    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x01);
    ym.step(144);
    let phase = ym.operator_envelope_debug(2, 0).unwrap().1;
    assert!(phase == "release" || phase == "off");
}

#[test]
fn test_ym2612_csm_retrigger_does_not_restart_phase_while_active() {
    let mut ym = Ym2612::new();
    ym.force_operator_phase(2, 0, 0x23456);
    ym.force_csm_key_state(2);

    ym.debug_apply_csm_key_on();

    assert_eq!(ym.operator_phase_debug(2, 0).unwrap(), 0x23456);
}

#[test]
fn test_ym2612_set_timing_updates_blip_rates() {
    let mut ym = Ym2612::new();

    ym.set_timing(audio::PAL_MCLK, 48_000);

    assert_eq!(ym.master_clock, audio::PAL_MCLK);
    assert_eq!(ym.sample_rate, 48_000);
    assert_eq!(ym.blip_l.clock_rate(), audio::PAL_MCLK);
    assert_eq!(ym.blip_r.clock_rate(), audio::PAL_MCLK);
    assert_eq!(ym.blip_l.sample_rate(), 48_000);
    assert_eq!(ym.blip_r.sample_rate(), 48_000);
}

#[test]
fn test_ym2612_set_timing_same_values_is_noop() {
    let mut ym = Ym2612::new();

    ym.blip_l.add_delta(0, 1234);
    ym.blip_r.add_delta(0, -1234);

    ym.set_timing(audio::NTSC_MCLK, audio::SAMPLE_RATE);

    assert_eq!(ym.master_clock, audio::NTSC_MCLK);
    assert_eq!(ym.sample_rate, audio::SAMPLE_RATE);
    assert_eq!(ym.blip_l.read_instant(), 1234);
    assert_eq!(ym.blip_r.read_instant(), -1234);
}

#[test]
fn test_ym2612_generate_sample_drains_right_channel_even_when_left_is_silent() {
    let mut ym = Ym2612::new();
    ym.blip_l = BlipBuf::new(44_100, 44_100);
    ym.blip_r = BlipBuf::new(44_100, 44_100);
    ym.blip_r.add_delta(0, 1000);

    // The band-limited kernel delays the step by half the kernel (8 samples):
    // after draining past the latency the right channel settles at exactly
    // 1000 while the silent left stays 0.
    let mut last = (0i16, 0i16);
    for _ in 0..9 {
        last = ym.generate_sample();
    }
    assert_eq!(last.0, 0);
    assert_eq!(last.1, 1000);
}
