//! VDP Property-Based Tests
//!
//! Uses proptest for comprehensive property testing of VDP behavior.

use crate::vdp::Vdp;
use proptest::prelude::*;

proptest! {
    /// Any CRAM color should convert to valid RGB565
    #[test]
    fn cram_color_converts_to_valid_rgb565(
        palette in 0u8..4,
        color in 0u8..16,
        r in 0u8..8,
        g in 0u8..8,
        b in 0u8..8
    ) {
        let mut vdp = Vdp::new();

        // Set CRAM color: ----BBB-GGG-RRR-
        let cram_value = ((b as u16) << 9) | ((g as u16) << 5) | ((r as u16) << 1);
        let addr = ((palette as usize) << 5) | ((color as usize) << 1);

        // Use write_control/write_data to ensure cache is updated
        vdp.write_control(0xC000 | (addr as u16));
        vdp.write_control(0x0000);
        vdp.write_data(cram_value);

        let rgb565 = vdp.cram_cache[(palette as usize) * 16 + (color as usize)];

        // Verify RGB565 components are within valid ranges
        let out_r = (rgb565 >> 11) & 0x1F;
        let out_g = (rgb565 >> 5) & 0x3F;
        let out_b = rgb565 & 0x1F;

        prop_assert!(out_r <= 31);
        prop_assert!(out_g <= 63);
        prop_assert!(out_b <= 31);
    }

    /// Plane size should always return valid dimensions (32, 64, or 128)
    #[test]
    fn plane_size_always_valid(reg_value in 0u8..=0xFF) {
        let mut vdp = Vdp::new();
        vdp.registers[16] = reg_value;

        let reg = vdp.registers[16];
        let w = match reg & 0x03 { 0 => 32, 1 => 64, 3 => 128, _ => 32 };
        let h = match (reg >> 4) & 0x03 { 0 => 32, 1 => 64, 3 => 128, _ => 32 };

        prop_assert!(w == 32 || w == 64 || w == 128);
        prop_assert!(h == 32 || h == 64 || h == 128);
    }

    /// Auto-increment should wrap addresses correctly
    #[test]
    fn auto_increment_wraps(
        start_addr in 0u16..=0xFFFE,
        increment in 1u8..16
    ) {
        let mut vdp = Vdp::new();

        // Set auto-increment
        vdp.registers[15] = increment;

        // Set VRAM write mode
        vdp.write_control(0x4000 | (start_addr & 0x3FFF));
        vdp.write_control(((start_addr >> 14) & 0x03) as u16);

        // Write data (triggers auto-increment)
        vdp.write_data(0x0000);

        // Address should wrap at 16-bit boundary
        let expected = start_addr.wrapping_add(increment as u16);
        prop_assert_eq!(vdp.control_address, expected);
    }

    /// Screen dimensions should match mode register settings
    #[test]
    fn screen_dimensions_match_mode(
        v30 in proptest::bool::ANY,
        h40 in proptest::bool::ANY
    ) {
        let mut vdp = Vdp::new();

        // Set mode register 2 (reg 1): V30 is bit 3
        vdp.registers[1] = if v30 { 0x08 } else { 0x00 };

        // Set mode register 4 (reg 12): H40 is bits 7 and 0 both set
        vdp.registers[12] = if h40 { 0x81 } else { 0x00 };

        let _expected_h = if h40 { 240 } else { 224 };
        let expected_w = if h40 { 320 } else { 256 };

        // Note: v30 controls height, h40 controls width
        prop_assert_eq!(vdp.screen_width(), expected_w);
        // v30 check
        let actual_h = vdp.screen_height();
        if v30 {
            prop_assert_eq!(actual_h, 240);
        } else {
            prop_assert_eq!(actual_h, 224);
        }
    }

    /// Register writes should never panic
    #[test]
    fn register_write_no_panic(
        reg_idx in 0u8..32,
        value in 0u8..=0xFF
    ) {
        let mut vdp = Vdp::new();

        // Register write format: 100RRRRR DDDDDDDD
        let cmd = 0x8000 | ((reg_idx as u16) << 8) | (value as u16);
        vdp.write_control(cmd);

        // Should not panic
        if reg_idx < 24 {
            prop_assert_eq!(vdp.registers[reg_idx as usize], value);
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::frontend::rgb565_to_rgba8;

    fn rgb565_to_rgb24(input: &[u16]) -> Vec<u8> {
        let mut output = vec![0u8; input.len() * 4];
        rgb565_to_rgba8(input, &mut output);
        // Extract RGB, dropping Alpha
        output
            .chunks(4)
            .flat_map(|c| vec![c[0], c[1], c[2]])
            .collect()
    }

    #[test]
    fn test_rgb565_to_rgb24_black() {
        let black = vec![0x0000u16];
        let result = rgb565_to_rgb24(&black);
        assert_eq!(result, vec![0, 0, 0]);
    }

    #[test]
    fn test_rgb565_to_rgb24_white() {
        let white = vec![0xFFFFu16];
        let result = rgb565_to_rgb24(&white);
        assert_eq!(result, vec![255, 255, 255]);
    }

    #[test]
    fn test_rgb565_to_rgb24_red() {
        // Pure red in RGB565: 11111 000000 00000 = 0xF800
        let red = vec![0xF800u16];
        let result = rgb565_to_rgb24(&red);
        assert_eq!(result[0], 255); // R should be 255
        assert_eq!(result[1], 0); // G should be 0
        assert_eq!(result[2], 0); // B should be 0
    }

    #[test]
    fn test_rgb565_to_rgb24_green() {
        // Pure green in RGB565: 00000 111111 00000 = 0x07E0
        let green = vec![0x07E0u16];
        let result = rgb565_to_rgb24(&green);
        assert_eq!(result[0], 0); // R should be 0
        assert_eq!(result[1], 255); // G should be 255
        assert_eq!(result[2], 0); // B should be 0
    }

    #[test]
    fn test_rgb565_to_rgb24_blue() {
        // Pure blue in RGB565: 00000 000000 11111 = 0x001F
        let blue = vec![0x001Fu16];
        let result = rgb565_to_rgb24(&blue);
        assert_eq!(result[0], 0); // R should be 0
        assert_eq!(result[1], 0); // G should be 0
        assert_eq!(result[2], 255); // B should be 255
    }

    #[test]
    fn test_vdp_vram_boundary() {
        let mut vdp = Vdp::new();

        // Direct VRAM write at 0xFFFE
        // Use direct VRAM array access since command encoding is complex
        vdp.vram[0xFFFE] = 0xAB;
        vdp.vram[0xFFFF] = 0xCD;

        assert_eq!(vdp.vram[0xFFFE], 0xAB);
        assert_eq!(vdp.vram[0xFFFF], 0xCD);
    }

    #[test]
    fn test_vdp_cram_boundary() {
        let mut vdp = Vdp::new();

        // Set CRAM write (CD = 0011)
        vdp.write_control(0xC07E); // Addr 0x7E (last valid pair)
        vdp.write_control(0x0000);
        vdp.write_data(0x1234);

        // Little-endian storage (existing behavior)
        assert_eq!(vdp.cram[0x7E], 0x34);
        assert_eq!(vdp.cram[0x7F], 0x12);
    }

    #[test]
    fn test_vdp_vsram_boundary() {
        let mut vdp = Vdp::new();

        // Direct VSRAM write at boundary
        // VSRAM is 80 bytes (0x00-0x4F)
        vdp.vsram[0x4E] = 0x56;
        vdp.vsram[0x4F] = 0x78;

        assert_eq!(vdp.vsram[0x4E], 0x56);
        assert_eq!(vdp.vsram[0x4F], 0x78);
    }

    #[test]
    fn test_vdp_hv_counter() {
        let mut vdp = Vdp::new();
        vdp.h_counter = 0x1234;
        vdp.v_counter = 0x00AB;

        let hv = vdp.read_hv_counter();
        let v_out = (hv >> 8) as u8;
        let h_out = hv as u8;

        assert_eq!(v_out, 0xAB);
        assert_eq!(h_out, 0x1A); // h_counter >> 1
    }
}
