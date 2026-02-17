#[cfg(test)]
mod tests {
    use crate::memory::bus::Bus;

    #[test]
    fn test_vdp_read_long_behavior() {
        let mut bus = Bus::new();
        bus.write_word(0xC00004, 0x8F02);
        bus.write_word(0xC00004, 0x4000);
        bus.write_word(0xC00004, 0x0000);

        bus.write_word(0xC00000, 0x1111);
        bus.write_word(0xC00000, 0x2222);
        bus.write_word(0xC00000, 0x3333);
        bus.write_word(0xC00000, 0x4444);

        bus.write_word(0xC00004, 0x0000);
        bus.write_word(0xC00004, 0x0000);

        let val = bus.read_long(0xC00000);

        println!("Read value: {:08X}", val);
        assert_eq!(val, 0x11112222, "Expected 0x11112222, got {:08X}", val);
    }

    #[test]
    fn test_vdp_write_long_behavior() {
        let mut bus = Bus::new();
        // Setup inc=2
        bus.write_word(0xC00004, 0x8F02);

        // Set VRAM address to 0.
        bus.write_word(0xC00004, 0x4000);
        bus.write_word(0xC00004, 0x0000);

        // Write Long: 0xAABBCCDD
        // Should be two writes: 0xAABB, 0xCCDD.
        bus.write_long(0xC00000, 0xAABBCCDD);

        // Read back
        bus.write_word(0xC00004, 0x0000);
        bus.write_word(0xC00004, 0x0000);

        let w1 = bus.read_word(0xC00000);
        let w2 = bus.read_word(0xC00000);

        println!("Write Long Readback: {:04X} {:04X}", w1, w2);
        assert_eq!(w1, 0xAABB, "Expected first word 0xAABB, got {:04X}", w1);
        assert_eq!(w2, 0xCCDD, "Expected second word 0xCCDD, got {:04X}", w2);
    }

    #[test]
    fn test_vdp_unaligned_read_long() {
        let mut bus = Bus::new();
        // Setup inc=2
        bus.write_word(0xC00004, 0x8F02);

        // Write VRAM at 0: 0x1111, 0x2222, 0x3333, 0x4444
        bus.write_word(0xC00004, 0x4000);
        bus.write_word(0xC00004, 0x0000);
        bus.write_word(0xC00000, 0x1111);
        bus.write_word(0xC00000, 0x2222);
        bus.write_word(0xC00000, 0x3333);
        bus.write_word(0xC00000, 0x4444);

        // Reset read address to 0
        bus.write_word(0xC00004, 0x0000);
        bus.write_word(0xC00004, 0x0000);

        // Read long at 0xC00002 (Unaligned)
        // Should read:
        // High word: VDP Data (0xC00002 -> same as 0xC00000) -> 0x1111
        // Low word: VDP Status (0xC00004) -> Status register value

        let val = bus.read_long(0xC00002);
        let high = (val >> 16) as u16;
        let low = (val & 0xFFFF) as u16;

        println!("Unaligned Read Long: {:08X}", val);
        assert_eq!(high, 0x1111, "High word should be VDP data 0x1111");
        // Status might change (e.g. VINT pending flag cleared on read?), but should be close to 0x3600
        assert_eq!(low & 0xFF00, 0x3600 & 0xFF00, "Low word should be status");

        // Verify subsequent read from data port (should be 0x2222)
        // Address advanced by 2.
        let val2 = bus.read_word(0xC00000);
        assert_eq!(val2, 0x2222, "Subsequent data read should be 0x2222");
    }
}
