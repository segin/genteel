use super::*;

#[test]
fn test_vram_access_out_of_bounds() {
    let mut vdp = Vdp::new();

    // Set Plane A base address to 0xE000 (Reg 2 = 0x38)
    // 0x38 << 10 = 0xE000 = 57344
    vdp.write_control(0x8238);

    // Set Plane Size to 128x128 (Reg 16 = 0x33)
    // 0011 0011 -> Size = 128x128
    vdp.write_control(0x9033);

    // Enable Display (Reg 1 bit 6 = 0x40)
    // Otherwise render_line returns early
    vdp.write_control(0x8140);

    // We need to trigger rendering on a line that causes out of bounds access.
    // We need `tile_v * plane_w + tile_h` to be large.
    // `tile_v` depends on `scrolled_v`. `scrolled_v = fetch_line + v_scroll`.
    // `tile_h` depends on `scrolled_h`. `scrolled_h = screen_x - h_scroll`.

    // Set VScroll to 1016.
    // `tile_v` = (1016 / 8) % 128 = 127.
    vdp.vsram[0] = (1016 >> 8) as u8;
    vdp.vsram[1] = (1016 & 0xFF) as u8;

    // Set HScroll for Plane A to 1.
    // `hs_base = hscroll_address()`. Default is 0.
    // `hs_addr` for Plane A is `hs_base` (0).
    // Write 1 to VRAM[0..1] (h_scroll).
    vdp.vram[0] = 0;
    vdp.vram[1] = 1;

    // Now `scrolled_h` for `screen_x=0` will be `0 - 1 = 65535`.
    // `tile_h` = (65535 / 8) % 128 = 8191 % 128 = 127.

    // With `tile_v` = 127 and `tile_h` = 127.
    // `nt_entry_addr` = `0xE000 + (127 * 128 + 127) * 2`.
    // = `57344 + 32766` = `90110`.

    // The wrapped address should be 90110 & 0xFFFF = 24574 (0x5FFE).
    let wrapped_addr = 24574;

    // Write nametable entry at wrapped address.
    // Entry: Priority=1, Palette=0, VFlip=0, HFlip=0, Tile=1.
    // 0x8001.
    vdp.vram[wrapped_addr] = 0x80;
    vdp.vram[wrapped_addr + 1] = 0x01;

    // Write Tile 1 pattern (at address 1 * 32 = 32).
    // We want color index 1.
    // Byte: 0x11 (pixels 0,1), 0x11 (pixels 2,3), etc.
    for i in 0..4 {
        vdp.vram[32 + i] = 0x11;
    }

    // Set CRAM color 1 (Palette 0) to Red (0xF800).
    // Palette 0, Color 1 is at index 1.
    vdp.cram_cache[1] = 0xF800;

    // Clear framebuffer
    vdp.framebuffer.fill(0);

    // This should NOT panic, and should render correctly.
    vdp.render_line(0);

    // Verify pixel at 0,0 is Red.
    assert_eq!(vdp.framebuffer[0], 0xF800, "Pixel at 0,0 should be Red (0xF800), indicating correct wrapping");
}
