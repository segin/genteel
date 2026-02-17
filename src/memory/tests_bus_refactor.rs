use super::bus::Bus;

#[test]
fn test_write_byte_rom() {
    let mut bus = Bus::new();
    bus.rom = vec![0; 1024];
    bus.write_byte(0x000100, 0xAA);
    assert_eq!(bus.rom[0x100], 0);
}

#[test]
fn test_write_byte_z80_ram() {
    let mut bus = Bus::new();
    bus.z80_bus_request = false;
    bus.write_byte(0xA00000, 0xAA);
    assert_eq!(bus.z80_ram[0], 0);

    bus.z80_bus_request = true;
    bus.write_byte(0xA00000, 0xAA);
    assert_eq!(bus.z80_ram[0], 0xAA);

    bus.write_byte(0xA01FFF, 0xBB);
    assert_eq!(bus.z80_ram[0x1FFF], 0xBB);
}

#[test]
fn test_write_byte_ym2612() {
    let mut bus = Bus::new();
    // Writing to 0xA04000 (Address port 0)
    // 0x27 is Timer Control.
    bus.write_byte(0xA04000, 0x27);
    // Writing to 0xA04001 (Data port 0)
    // Write 0x00 to Reg 0x27.
    bus.write_byte(0xA04001, 0x00);

    // Check if FM registers updated.
    // Bank 0, Reg 0x27 is at registers[0][0x27].
    assert_eq!(bus.apu.fm.registers[0][0x27], 0x00);

    // Write 0x30 to Reg 0x27.
    bus.write_byte(0xA04000, 0x27);
    bus.write_byte(0xA04001, 0x30);
    assert_eq!(bus.apu.fm.registers[0][0x27], 0x30);

    // Port 1 (0xA04002 / 0xA04003)
    // Write to Reg 0x30 in Bank 1.
    bus.write_byte(0xA04002, 0x30);
    bus.write_byte(0xA04003, 0x42);
    assert_eq!(bus.apu.fm.registers[1][0x30], 0x42);
}

#[test]
fn test_write_byte_z80_bank() {
    let mut bus = Bus::new();
    // 0xA06000
    // bus.z80_bank_addr starts at 0.
    // write 1 -> bit=1<<15.
    bus.write_byte(0xA06000, 0x01);
    assert_eq!(bus.z80_bank_addr, 0x8000);
    assert_eq!(bus.z80_bank_bit, 1);

    bus.write_byte(0xA06000, 0x01); // bit=1<<16
    assert_eq!(bus.z80_bank_addr, 0x18000); // 0x8000 | 0x10000
    assert_eq!(bus.z80_bank_bit, 2);
}

#[test]
fn test_write_byte_io() {
    let mut bus = Bus::new();

    bus.write_byte(0xA10009, 0xFF); // Control port 1
    let val = bus.read_byte(0xA10009);
    assert_eq!(val, 0xFF);
}

#[test]
fn test_write_byte_z80_control() {
    let mut bus = Bus::new();

    // Bus Request
    bus.write_byte(0xA11100, 0x01);
    assert!(bus.z80_bus_request);
    bus.write_byte(0xA11100, 0x00);
    assert!(!bus.z80_bus_request);

    // Reset
    bus.write_byte(0xA11200, 0x00); // Active Low
    assert!(bus.z80_reset);
    // Reset clears bank bit?
    bus.z80_bank_bit = 5;
    bus.write_byte(0xA11200, 0x00);
    assert!(bus.z80_reset);
    assert_eq!(bus.z80_bank_bit, 0);

    bus.write_byte(0xA11200, 0x01);
    assert!(!bus.z80_reset);
}

#[test]
fn test_write_byte_vdp() {
    let mut bus = Bus::new();
    // Data port (byte write) -> Placeholder, should do nothing.
    bus.vdp.vram[0] = 0;
    bus.write_byte(0xC00000, 0xAA);
    // If it did something, it might write to VRAM or change control state.
    // But it's a placeholder.
    assert_eq!(bus.vdp.vram[0], 0);

    // Control port (byte write) -> Placeholder.
    let old_code = bus.vdp.control_code;
    bus.write_byte(0xC00004, 0xAA);
    assert_eq!(bus.vdp.control_code, old_code);

    // PSG
    // 0xC00011
    // Initial volume is 0x0F (Silent).
    bus.write_byte(0xC00011, 0x90); // Latch Ch0 Vol to 0 (Loud)
    assert_eq!(bus.apu.psg.tones[0].volume, 0);
}

#[test]
fn test_write_byte_work_ram() {
    let mut bus = Bus::new();
    bus.write_byte(0xE00000, 0x12);
    assert_eq!(bus.work_ram[0], 0x12);

    bus.write_byte(0xFFFFFF, 0x34);
    assert_eq!(bus.work_ram[0xFFFF], 0x34);
}
