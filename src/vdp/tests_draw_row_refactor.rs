use super::*;
use super::render::RenderOps;

#[test]
fn test_draw_full_tile_row_refactor() {
    let mut vdp = Vdp::new();
    // Setup palette 0, color 1 = Red
    vdp.cram_cache[1] = 0xF800;

    // Setup a tile pattern in VRAM. Tile index 0.
    // Bytes 0-3: 0x11, 0x00, 0x00, 0x00.
    // Pixel 0, 1 are color 1. Others 0.
    vdp.vram[0] = 0x11;
    vdp.vram[1] = 0x00;
    vdp.vram[2] = 0x00;
    vdp.vram[3] = 0x00;

    // Entry: Palette 0 (0), Priority 0, Flip 0. Tile 0.
    // 0x0000.
    let entry = 0x0000;
    let pixel_v = 0; // Top row of tile
    let dest_idx = 0; // Start of framebuffer

    vdp.draw_full_tile_row(entry, pixel_v, dest_idx);

    // Check pixels
    assert_eq!(vdp.framebuffer[0], 0xF800, "Pixel 0");
    assert_eq!(vdp.framebuffer[1], 0xF800, "Pixel 1");
    assert_eq!(vdp.framebuffer[2], 0x0000, "Pixel 2");
}

#[test]
fn test_draw_full_tile_row_bounds_safe() {
    let mut vdp = Vdp::new();
    // Try to draw at end of framebuffer
    let dest_idx = vdp.framebuffer.len() - 4; // Not enough space for 8 pixels
    // Should not panic
    vdp.draw_full_tile_row(0, 0, dest_idx);
}

#[test]
fn test_draw_full_tile_row_hflip() {
    let mut vdp = Vdp::new();
    vdp.cram_cache[1] = 0xF00; // Red
    vdp.cram_cache[2] = 0x0F0; // Green

    // Tile 0: 0x12, 0x00...
    // Pixels: 1, 2, 0...
    vdp.vram[0] = 0x12;
    vdp.vram[1] = 0x00;
    vdp.vram[2] = 0x00;
    vdp.vram[3] = 0x00;

    // Entry with H-Flip (0x0800)
    let entry = 0x0800;

    vdp.draw_full_tile_row(entry, 0, 0);

    // H-Flip:
    // Original: 1, 2, 0, 0, 0, 0, 0, 0
    // Flipped:  0, 0, 0, 0, 0, 0, 2, 1

    assert_eq!(vdp.framebuffer[7], 0xF00, "Pixel 7 should be Red (1)");
    assert_eq!(vdp.framebuffer[6], 0x0F0, "Pixel 6 should be Green (2)");
    assert_eq!(vdp.framebuffer[0], 0x000, "Pixel 0 should be empty");
}
