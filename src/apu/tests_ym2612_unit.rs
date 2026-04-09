use super::ym2612::{Bank, Ym2612};
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

    ym.step(32); // 32 * 7 = 224 mclocks
    assert_eq!(ym.read_status() & 0x80, 0); // Busy flag should be cleared
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

    ym.step(20);
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

    ym.step(200);
    assert_ne!(ym.read_status() & 0x02, 0); // Timer B expired
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
