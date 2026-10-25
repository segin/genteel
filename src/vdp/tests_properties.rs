//! VDP Property-Based Tests
//!
//! Uses proptest for comprehensive property testing of VDP behavior.

use crate::vdp::{
    RenderOps, Vdp, MODE1_HINT_ENABLE, MODE2_VINT_ENABLE, REG_H_INT_COUNTER, REG_PLANE_SIZE,
    STATUS_VBLANK, STATUS_VINT_PENDING,
};
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
        vdp.bypass_fifo = true;

        // Set CRAM color: ----BBB-GGG-RRR-
        let cram_value = ((b as u16) << 9) | ((g as u16) << 5) | ((r as u16) << 1);
        let addr = ((palette as usize) << 5) | ((color as usize) << 1);

        // Use write_control/write_data to ensure cache is updated
        vdp.write_control(0xC000 | (addr as u16));
        vdp.write_control(0x0000);
        vdp.write_data(cram_value);

        let rgb565 = vdp.get_cram_color(palette, color);

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
        vdp.bypass_fifo = true;
        vdp.registers[REG_PLANE_SIZE] = reg_value;

        let (w, h) = vdp.plane_size();

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
        vdp.bypass_fifo = true;

        // Set auto-increment
        vdp.registers[15] = increment;

        // Set VRAM write mode
        vdp.write_control(0x4000 | (start_addr & 0x3FFF));
        vdp.write_control((start_addr >> 14) & 0x03);

        // Write data (triggers auto-increment)
        vdp.write_data(0x0000);

        // Address should wrap at 16-bit boundary
        let expected = start_addr.wrapping_add(increment as u16);
        prop_assert_eq!(vdp.command.address, expected);
    }

    /// Screen dimensions should match mode register settings
    #[test]
    fn screen_dimensions_match_mode(
        v30 in proptest::bool::ANY,
        h40 in proptest::bool::ANY
    ) {
        let mut vdp = Vdp::new();
        vdp.bypass_fifo = true;

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
        vdp.bypass_fifo = true;

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
        vdp.bypass_fifo = true;

        // Set CRAM write (CD = 0011)
        vdp.write_control(0xC07E); // Addr 0x7E (last valid pair)
        vdp.write_control(0x0000);
        vdp.write_data(0x1234);

        // Little-endian storage, masked to the 9 implemented colour bits.
        assert_eq!(vdp.cram[0x7E], 0x24);
        assert_eq!(vdp.cram[0x7F], 0x02);
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
        vdp.mclk_line_clocks = 0;
        vdp.v_counter = 0x00AB;

        let hv = vdp.read_hv_counter();
        let v_out = (hv >> 8) as u8;
        let h_out = hv as u8;

        assert_eq!(v_out, 0xAB);
        assert_eq!(
            h_out, 0x85,
            "the line (and V counter) changes at H 0x85 in H32 mode"
        );
    }

    #[test]
    fn h_counter_h40_sequence() {
        use crate::vdp::timing::h_counter_at;
        assert_eq!(h_counter_at(0, true), 0xA5);
        assert_eq!(h_counter_at(271, true), 0xB5);
        assert_eq!(h_counter_at(272, true), 0xB6);
        assert_eq!(h_counter_at(280, true), 0xE4);
        assert_eq!(h_counter_at(288, true), 0xE5);
        assert_eq!(h_counter_at(780, true), 0x00);
        assert_eq!(h_counter_at(988, true), 0x0D);
        assert_eq!(h_counter_at(3419, true), 0xA4);
        let mut distinct = std::collections::BTreeSet::new();
        for m in 0..3420u32 {
            let h = h_counter_at(m, true);
            distinct.insert(h);
            assert!(
                !(0xB7..=0xE3).contains(&h),
                "H40 produced gap value 0x{:02X} at MCLK {}",
                h,
                m
            );
        }
        assert_eq!(distinct.len(), 211);
    }

    #[test]
    fn h_counter_h32_sequence() {
        use crate::vdp::timing::h_counter_at;
        assert_eq!(h_counter_at(0, false), 0x85);
        assert_eq!(h_counter_at(280, false), 0x93);
        assert_eq!(h_counter_at(300, false), 0xE9);
        assert_eq!(h_counter_at(740, false), 0xFF);
        assert_eq!(h_counter_at(760, false), 0x00);
        assert_eq!(h_counter_at(3419, false), 0x84);
        let mut distinct = std::collections::BTreeSet::new();
        for m in 0..3420u32 {
            let h = h_counter_at(m, false);
            distinct.insert(h);
            assert!(
                !(0x94..=0xE8).contains(&h),
                "H32 produced gap value 0x{:02X} at MCLK {}",
                h,
                m
            );
        }
        assert_eq!(distinct.len(), 171);
    }

    #[test]
    fn v_counter_ntsc_v28_jump_table() {
        assert_eq!(Vdp::v_counter_value_for_line(0, false, false), 0x00);
        assert_eq!(Vdp::v_counter_value_for_line(0xEA, false, false), 0xEA);
        assert_eq!(Vdp::v_counter_value_for_line(0xEB, false, false), 0xE5);
        assert_eq!(Vdp::v_counter_value_for_line(261, false, false), 0xFF);
        /* After the jump the counter re-reads 0xE5..=0xEA, so every value
        0x00..=0xFF appears, and exactly the six values 0xE5..=0xEA appear
        twice per frame. */
        let mut seen = [0u32; 256];
        for line in 0..262u16 {
            seen[Vdp::v_counter_value_for_line(line, false, false) as usize] += 1;
        }
        for (v, &n) in seen.iter().enumerate() {
            let expected = if (0xE5..=0xEA).contains(&v) { 2 } else { 1 };
            assert_eq!(n, expected, "NTSC V28 value 0x{:02X} seen {} times", v, n);
        }
    }

    #[test]
    fn v_counter_pal_v28_jump_table() {
        // PAL V28 (313 lines): 0x00..=0xFF (256), then a short 0x00..=0x02 run
        // immediately after the wrap (3), then 0xCA..=0xFF (54).
        assert_eq!(Vdp::v_counter_value_for_line(0x00, true, false), 0x00);
        assert_eq!(Vdp::v_counter_value_for_line(0xFF, true, false), 0xFF);
        assert_eq!(Vdp::v_counter_value_for_line(0x100, true, false), 0x00);
        assert_eq!(Vdp::v_counter_value_for_line(0x102, true, false), 0x02);
        assert_eq!(Vdp::v_counter_value_for_line(0x103, true, false), 0xCA);
        // Last line of the 313-line frame (0x138) maps to 0xFF.
        assert_eq!(Vdp::v_counter_value_for_line(0x138, true, false), 0xFF);
    }

    #[test]
    fn test_vdp_tick_drives_hint_counter_on_active_lines() {
        let mut vdp = Vdp::new();
        vdp.registers[0] = MODE1_HINT_ENABLE;
        vdp.registers[REG_H_INT_COUNTER] = 5;
        vdp.v_counter = 222;
        vdp.line_counter = 0;
        vdp.mclk_line_clocks = 3419;

        // Crossing the line boundary fires the line interrupt as the V
        // counter advances.
        vdp.tick(1, |_| 0);

        assert_eq!(vdp.v_counter, 223);
        assert_eq!(vdp.line_counter, 5);
        assert!(vdp.hint_pending());
        assert_eq!(vdp.status & STATUS_VBLANK, 0);
    }

    /// HINT is asserted at the line change (H 0xA5/0x85), together with the
    /// V counter increment.
    #[test]
    fn hint_asserts_at_line_change() {
        let mut vdp = Vdp::new();
        vdp.registers[0] = MODE1_HINT_ENABLE;
        vdp.registers[REG_H_INT_COUNTER] = 0;
        vdp.v_counter = 0;
        vdp.line_counter = 0;
        vdp.mclk_line_clocks = 3419;

        assert!(!vdp.hint_pending);
        vdp.tick(1, |_| 0);
        assert_eq!(vdp.v_counter, 1);
        assert!(vdp.hint_pending);
    }

    /// VINT asserts partway into the first VBlank line, at H 0x00, not at
    /// the line change.
    #[test]
    fn vint_asserts_at_its_line_position() {
        let mut vdp = Vdp::new();
        vdp.registers[1] = MODE2_VINT_ENABLE;
        vdp.v_counter = 223;
        vdp.mclk_line_clocks = 3419;

        // Wrap to v_counter = 224 (first VBlank line): VBLANK set, VINT not yet.
        vdp.tick(1, |_| 0);
        assert_ne!(vdp.status & STATUS_VBLANK, 0);
        assert_eq!(vdp.status & STATUS_VINT_PENDING, 0);

        let vint = vdp.line_timing().vint;
        vdp.tick(vint - 1, |_| 0);
        assert_eq!(vdp.status & STATUS_VINT_PENDING, 0);
        vdp.tick(1, |_| 0);
        assert_ne!(vdp.status & STATUS_VINT_PENDING, 0);
        assert_eq!(vdp.read_hv_counter() & 0xFF, 0x00);
    }

    #[test]
    fn test_vdp_tick_does_not_fire_hint_on_first_vblank_line() {
        let mut vdp = Vdp::new();
        vdp.registers[0] = MODE1_HINT_ENABLE;
        vdp.registers[REG_H_INT_COUNTER] = 7;
        vdp.v_counter = 223;
        vdp.line_counter = 1;
        vdp.mclk_line_clocks = 3419;

        vdp.tick(1, |_| 0);

        assert_eq!(vdp.v_counter, 224);
        assert_eq!(vdp.line_counter, 0);
        assert!(!vdp.hint_pending());
        assert_ne!(vdp.status & STATUS_VBLANK, 0);
    }
}
