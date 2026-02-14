use super::*;

#[test]
fn test_vram_access_out_of_bounds() {
    let mut vdp = Vdp::new();

    // Set Plane A base address to 0xE000 (Reg 2 = 0x38)
    // 0x38 << 10 = 0xE000 = 57344
    vdp.write_control(0x8238);

    // Set Plane Size to 128x128 (Reg 16 = 0x33)
    // 0011 0011 -> 0x33
    // But Reg 16 only uses bits 0-1 and 4-5.
    // 0011 0011 -> Size = 128x128
    vdp.write_control(0x9033);

    // Enable Display (Reg 1 bit 6 = 0x40)
    // Otherwise render_line returns early
    vdp.write_control(0x8140);

    // We need to trigger rendering on a line that causes out of bounds access.
    // We need `tile_v * plane_w + tile_h` to be large.
    // `tile_v` depends on `scrolled_v`. `scrolled_v = fetch_line + v_scroll`.
    // `tile_h` depends on `scrolled_h`. `scrolled_h = screen_x - h_scroll`.

    // Let's set VScroll to 0 (default).
    // Let's set HScroll to 0 (default).

    // `fetch_line` = `draw_line` (assuming not PAL/V30).
    // Let's pick `draw_line` = 200.

    // To get `tile_v` large, we need `scrolled_v` large.
    // `scrolled_v` wraps at `plane_h * 8`.
    // If plane_h is 128, max pixel height is 1024.

    // Wait, `tile_v = (scrolled_v / 8) % plane_h`.
    // If we want `tile_v` = 127. We need `scrolled_v` around 127 * 8 = 1016.
    // VDP height is usually 224 or 240. So `scrolled_v` (without scroll) is max 239.
    // So `tile_v` will be at most 239/8 = 29.

    // To get `tile_v` higher, we need to use VScroll.
    // VSRAM entry 0 (Plane A).
    // `vdp.vsram[0]`, `vdp.vsram[1]`.
    // Let's set VScroll to a value that makes `scrolled_v` point to bottom of plane.
    // If we want `tile_v` = 127. `scrolled_v` should be 1016.
    // If `fetch_line` is 0. `v_scroll` should be 1016.
    // VSRAM is 10 bits? Masked with 0x3FF (1023).
    // So we can set `v_scroll` to 1016.

    vdp.vsram[0] = (1016 >> 8) as u8;
    vdp.vsram[1] = (1016 & 0xFF) as u8;

    // Now `tile_v` will be around 127.

    // To get `tile_h` large, we need `scrolled_h` large.
    // `scrolled_h = screen_x - h_scroll`.
    // `screen_x` goes from 0 to 319.
    // If `h_scroll` is 0, `tile_h` goes from 0 to 39.

    // We want `tile_h` = 127.
    // We need `scrolled_h` around 1016.
    // `scrolled_h` is `u16`. Wrapping subtract.
    // If we want `scrolled_h` = 1016.
    // And `screen_x` = 0.
    // `0 - h_scroll` = 1016 (mod 1024? No, hscroll is 10 bits masked).
    // The code:
    // `let scrolled_h = (screen_x as u16).wrapping_sub(h_scroll);`
    // `let tile_h = (scrolled_h as usize / 8) % plane_w;`
    // So if `screen_x` = 0. `0 - h_scroll` = large.
    // If `h_scroll` is small positive, `scrolled_h` becomes large (near 65535).
    // `tile_h` = `(65535 / 8) % 128` = `8191 % 128` = `127`.

    // So setting `h_scroll` = 1 should be enough to get high `tile_h` for `screen_x=0`.

    // Set HScroll for Plane A.
    // `hs_base = hscroll_address()`. Default is 0.
    // `hs_addr` for Plane A is `hs_base`.
    // We need to write to VRAM at `hs_base`.
    // Let's assume `hs_base` is 0 for now (default Reg 13 = 0).
    // Write 1 to VRAM[0..1].
    // `h_scroll` = `vram[0] << 8 | vram[1]`.
    // Wait, `h_scroll` calculation:
    // `let hi = self.vram[hs_addr]; let lo = self.vram[hs_addr + 1];`
    // We want `h_scroll` = 1.
    vdp.vram[0] = 0;
    vdp.vram[1] = 1;

    // Now `scrolled_h` for `screen_x=0` will be `0 - 1 = 65535`.
    // `tile_h` = 127.

    // With `tile_v` = 127 and `tile_h` = 127.
    // `nt_entry_addr` = `0xE000 + (127 * 128 + 127) * 2`.
    // = `57344 + 32766` = `90110`.

    // This should panic.
    vdp.render_line(0);
}
