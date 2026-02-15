use super::*;

#[test]
fn test_vram_access_out_of_bounds() {
    // Regression test: Ensure that rendering with high scroll values causing
    // nametable address calculation to exceed 64KB does not cause a panic.
    let mut vdp = Vdp::new();

    // 1. Configure VDP
    // Set Plane A base address to 0xE000 (Reg 2 = 0x38) -> High in VRAM
    vdp.write_control(0x8238);

    // Set Plane Size to 128x128 (Reg 16 = 0x33) -> Max plane size
    vdp.write_control(0x9033);

    // Enable Display (Reg 1 bit 6 = 0x40) -> Required for rendering
    vdp.write_control(0x8140);

    // 2. Set Scroll values to trigger high address calculation
    // Set VScroll for Plane A to 1016 (near end of 1024 pixel height)
    vdp.vsram[0] = (1016 >> 8) as u8;
    vdp.vsram[1] = (1016 & 0xFF) as u8;

    // Set HScroll for Plane A to 1
    // HScroll is read from VRAM at hscroll_address (default 0).
    // Writing 1 to VRAM[0..1] sets HScroll to 1.
    // screen_x=0 minus HScroll=1 gives a large wrapped coordinate (~65535).
    vdp.vram[0] = 0;
    vdp.vram[1] = 1;

    // 3. Render a line
    // With tile_v=127 and tile_h=127, the address formula would be:
    // 0xE000 + (127 * 128 + 127) * 2 = 90110
    // This is > 65536. The VDP implementation must mask this to avoid OOB panic.
    vdp.render_line(0);
}
