use super::bus::Bus;
use proptest::prelude::*;

proptest! {
    // Test that ROM reading is consistent across different access sizes
    #[test]
    fn prop_rom_read_consistency(addr in 0..0x3FFF0u32) {
        let mut bus = Bus::new();
        // Create dummy ROM with some pattern
        let mut rom = vec![0u8; 0x40000];
        for i in 0..rom.len() {
            rom[i] = (i & 0xFF) as u8;
        }
        bus.load_rom(&rom);

        let b0 = bus.read_byte(addr);
        let b1 = bus.read_byte(addr + 1);
        let word = bus.read_word(addr);

        prop_assert_eq!(word, ((b0 as u16) << 8) | (b1 as u16));
    }

    // Test WRAM mirroring
    #[test]
    fn prop_wram_mirroring(addr in 0..0xFFFFu32, val in 0..=255u8) {
        let mut bus = Bus::new();
        let base_addr = 0xFF0000;
        let mirror_addr = 0xE00000 + addr;

        bus.write_byte(base_addr + addr, val);
        prop_assert_eq!(bus.read_byte(mirror_addr), val);
    }

    // Test Endianness consistency
    #[test]
    fn prop_endianness_consistency(val in any::<u32>()) {
        let mut bus = Bus::new();
        let addr = 0xFF0000;
        bus.write_long(addr, val);

        prop_assert_eq!(bus.read_byte(addr), (val >> 24) as u8);
        prop_assert_eq!(bus.read_byte(addr + 1), (val >> 16) as u8);
        prop_assert_eq!(bus.read_byte(addr + 2), (val >> 8) as u8);
        prop_assert_eq!(bus.read_byte(addr + 3), (val & 0xFF) as u8);
    }

    // Test Z80 RAM basic access
    #[test]
    fn prop_z80_ram_access(addr in 0xA00000..0xA01FFFu32, val in 0..=255u8) {
        let mut bus = Bus::new();
        // Request bus
        bus.write_byte(0xA11100, 0x01);

        bus.write_byte(addr, val);
        prop_assert_eq!(bus.read_byte(addr), val);
    }

    // Test I/O Control Register Access
    #[test]
    fn prop_io_control_access(val in 0..=255u8) {
        let mut bus = Bus::new();
        let addr = 0xA10009; // Control port 1
        bus.write_byte(addr, val);
        prop_assert_eq!(bus.read_byte(addr), val);
    }
}

// Security and Stability tests
#[test]
fn test_vdp_status_read_consistency() {
    let mut bus = Bus::new();
    // VDP status is at 0xC00004
    let s1 = bus.read_word(0xC00004);
    let s2 = bus.read_word(0xC00004);
    // On Genesis, status bits like FIFO empty/full and VBlank might change,
    // but in a single-threaded test with no stepping, it should be stable.
    assert_eq!(s1, s2);
}
