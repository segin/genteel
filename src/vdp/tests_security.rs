use super::*;

#[test]
fn test_vdp_plane_rendering_oob_safety() {
    let mut vdp = Vdp::new();

    // 1. Enable Display: Reg 1, bit 6 (0x40)
    vdp.registers[1] = 0x40;

    // 2. Set Plane A Table Base to maximum possible: 0xE000
    // Reg 2: bits 5-3 are bits 15-13 of address. (val & 0x38) << 10.
    // 0x38 = 0011 1000 binary.
    // 0x38 << 10 = 0xE000.
    vdp.registers[2] = 0x38;

    // 3. Set Plane Size to maximum: 128x128
    // Reg 16: bits 1-0 for width (00=32, 01=64, 11=128), bits 5-4 for height.
    // 128x128 -> Width=11 (3), Height=11 (3).
    // 0x33 = 0011 0011.
    vdp.registers[16] = 0x33;

    // 4. Set Vertical Scroll to maximum to reach bottom of the plane.
    // VSRAM index 0 (Plane A). Value 1023 (0x3FF).
    // This adds to line index.
    // line 0 + 1023 = 1023.
    // 1023 / 8 = 127 (Max tile_v for 128-height plane).
    vdp.vsram[0] = 0x03;
    vdp.vsram[1] = 0xFF;

    // 5. Render line 0
    // tile_v = 127. plane_w = 128.
    // Base address offset = (127 * 128) * 2 = 16256 * 2 = 32512.
    // name_table_base = 0xE000 = 57344.
    // Final address = 57344 + 32512 = 89856 (0x15F00).
    // This exceeds VRAM size (0x10000), so without masking it would panic.

    vdp.render_line(0);
}
