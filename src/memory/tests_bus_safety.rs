use super::bus::Bus;

#[test]
fn test_rom_boundary_access_safety() {
    let mut bus = Bus::new();
    // Create a ROM of 512 bytes to avoid resizing padding (which fills with 0)
    let mut rom_data = vec![0; 512];
    rom_data[511] = 0xAA;
    bus.load_rom(&rom_data);

    // Test read_word at 510 (safe)
    // idx=510. idx+1=511. len=512. Safe.
    // rom[510]=0, rom[511]=0xAA. -> 0x00AA.
    assert_eq!(bus.read_word(510), 0x00AA);

    // Test read_word at 511 (partial read)
    // idx=511. idx+1=512. len=512.
    // 512 < 512 is false.
    // 511 < 512 is true.
    // High=rom[511]=0xAA. Low=0xFF. -> 0xAAFF.
    assert_eq!(bus.read_word(511), 0xAAFF);

    // Test read_word at 512 (out of bounds)
    // idx=512.
    // returns 0xFFFF.
    assert_eq!(bus.read_word(512), 0xFFFF);
}
