use super::ym2612::{Bank, Ym2612};

#[test]
fn test_ym2612_all_channels_enable() {
    let mut ym = Ym2612::new();

    // Enable all 6 channels by setting frequency and volume
    for ch in 0..6 {
        let (bank, offset) = if ch < 3 {
            (Bank::Bank0, ch)
        } else {
            (Bank::Bank1, ch - 3)
        };

        // Freq setting
        ym.write_addr(bank, 0xA0 + offset as u8);
        ym.write_data_bank(bank, 0x55);
        ym.write_addr(bank, 0xA4 + offset as u8);
        ym.write_data_bank(bank, 0x22);

        // Volume setting (TL for Op 4)
        ym.write_addr(bank, 0x4C + offset as u8);
        ym.write_data_bank(bank, 0x00); // Max volume
    }

    // Generate samples and verify non-zero
    let (l, r) = ym.generate_sample();
    assert!(
        l != 0 || r != 0,
        "Samples should be non-zero when channels are active"
    );
}

#[test]
fn test_ym2612_dac_panning() {
    let mut ym = Ym2612::new();

    // Enable DAC: Reg 0x2B bit 7
    ym.write_addr(Bank::Bank0, 0x2B);
    ym.write_data_bank(Bank::Bank0, 0x80);

    // DAC Data: Reg 0x2A. Let's set it to 0xFF (Max positive)
    ym.write_addr(Bank::Bank0, 0x2A);
    ym.write_data_bank(Bank::Bank0, 0xFF);

    // Panning for Ch6: Bank 1, Reg 0xB6
    // Initially pan=0 (mixed) in current impl?
    // Wait, generate_sample: `let pan = self.registers[1][0xB6]; if (pan & 0x80) != 0 { left += dac_f; }`
    // So if pan is 0, it doesn't add to left/right for DAC?

    // Set Pan Left Only: 0x80
    ym.write_addr(Bank::Bank1, 0xB6);
    ym.write_data_bank(Bank::Bank1, 0x80);

    let (l, r) = ym.generate_sample();
    assert!(l > 0, "Left should be positive: {}", l);
    assert_eq!(r, 0, "Right should be zero: {}", r);

    // Set Pan Right Only: 0x40
    ym.write_addr(Bank::Bank1, 0xB6);
    ym.write_data_bank(Bank::Bank1, 0x40);
    let (l, r) = ym.generate_sample();
    assert_eq!(l, 0, "Left should be zero: {}", l);
    assert!(r > 0, "Right should be positive: {}", r);
}

#[test]
fn test_ym2612_timer_ab_simultaneous() {
    let mut ym = Ym2612::new();

    // Timer A: period 144
    ym.write_addr(Bank::Bank0, 0x24);
    ym.write_data_bank(Bank::Bank0, 0xFF);
    ym.write_addr(Bank::Bank0, 0x25);
    ym.write_data_bank(Bank::Bank0, 0x03);

    // Timer B: period 2304
    ym.write_addr(Bank::Bank0, 0x26);
    ym.write_data_bank(Bank::Bank0, 0xFF);

    // Enable both with flags: 0x01 | 0x02 | 0x04 | 0x08 = 0x0F
    ym.write_addr(Bank::Bank0, 0x27);
    ym.write_data_bank(Bank::Bank0, 0x0F);

    // Step enough for Timer B (330 68k cycles)
    ym.step(330);

    assert_eq!(ym.status & 0x03, 0x03, "Both timers should have fired");
}
