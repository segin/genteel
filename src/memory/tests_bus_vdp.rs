
#[cfg(test)]
mod tests {
    use crate::memory::bus::Bus;

    #[test]
    fn test_bus_read_vdp_status_bytes() {
        let mut bus = Bus::new();

        // Initial status is 0x3600 (FIFO empty, etc)
        // High byte: 0x36
        // Low byte: 0x00

        let high = bus.read_byte(0xC00004);
        assert_eq!(high, 0x36, "High byte at 0xC00004 should be 0x36");

        let low = bus.read_byte(0xC00005);
        assert_eq!(low, 0x00, "Low byte at 0xC00005 should be 0x00"); // This should fail currently
    }

    #[test]
    fn test_bus_read_vdp_status_clears_pending() {
        let mut bus = Bus::new();

        // Write first word of command to set pending
        bus.write_word(0xC00004, 0x4000);
        assert!(bus.vdp.is_control_pending(), "Should be pending after write");

        // Read byte at 0xC00004 should clear pending
        bus.read_byte(0xC00004);
        assert!(!bus.vdp.is_control_pending(), "Should NOT be pending after reading 0xC00004");

        // Reset and try with 0xC00005
        bus.vdp.reset();
        bus.write_word(0xC00004, 0x4000);
        assert!(bus.vdp.is_control_pending());

        bus.read_byte(0xC00005);
        assert!(!bus.vdp.is_control_pending(), "Should NOT be pending after reading 0xC00005");
    }
}
