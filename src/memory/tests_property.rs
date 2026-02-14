#[cfg(test)]
use crate::memory::bus::Bus;
use proptest::prelude::*;

proptest! {
    // Test that reading from ROM works within bounds and returns FF outside
    #[test]
    fn prop_rom_read_consistency(
        rom_size in 512usize..10000,
        addr in 0..0x400000u32
    ) {
        let mut bus = Bus::new();
        let rom_data = vec![0xAA; rom_size];
        bus.load_rom(&rom_data);

        let val = bus.read_byte(addr);
        if (addr as usize) < rom_data.len() {
             prop_assert_eq!(val, 0xAA);
        } else {
             prop_assert_eq!(val, 0xFF);
        }
    }

    // Test WRAM mirroring at 0xE00000 - 0xFFFFFF
    #[test]
    fn prop_wram_mirroring(
        addr in 0xE00000..0xFFFFFFu32,
        val in 0..=255u8
    ) {
        let mut bus = Bus::new();
        bus.write_byte(addr, val);

        // Read back from original address
        prop_assert_eq!(bus.read_byte(addr), val);

        // Calculate offset in 64KB block
        let offset = addr & 0xFFFF;

        // Check base address 0xFF0000
        prop_assert_eq!(bus.read_byte(0xFF0000 + offset), val);

        // Check mirror range start 0xE00000
        prop_assert_eq!(bus.read_byte(0xE00000 + offset), val);
    }

    // Test Word and Long read/write consistency (check endianness)
    #[test]
    fn prop_endianness_consistency(addr in 0xE00000..0xFFFFFCu32, val in 0..=0xFFFFFFFFu32) {
        let mut bus = Bus::new();
        bus.write_long(addr, val);

        // Read back as long
        prop_assert_eq!(bus.read_long(addr), val);

        // Read back as words
        let high_word = (val >> 16) as u16;
        let low_word = (val & 0xFFFF) as u16;
        prop_assert_eq!(bus.read_word(addr), high_word);
        prop_assert_eq!(bus.read_word(addr + 2), low_word);

        // Read back as bytes
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

    // Test I/O Control Port write/read (writable registers)
    #[test]
    fn prop_io_control_access(val in 0..=255u8) {
        let mut bus = Bus::new();
        // Port 1 Control is at 0xA10009
        bus.write_byte(0xA10009, val);
        prop_assert_eq!(bus.read_byte(0xA10009), val);
    }
}

// Regular unit tests that don't need proptest arguments
#[test]
fn test_vdp_status_read_consistency() {
    let mut bus = Bus::new();
    let status = bus.read_word(0xC00004);
    // Status should have FIFO bits set by default in our stub
    assert_eq!(status & 0x3600, 0x3600);
}
