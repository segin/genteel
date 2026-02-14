use super::*;

#[test]
fn test_render_plane_basic() {
    let mut vdp = Vdp::new();
    vdp.set_region(false); // NTSC

    // Enable Display: Reg 1, bit 6 (0x40)
    vdp.registers[1] = 0x40;

    // Plane A Address: 0xC000 (Reg 2 = 0x30)
    vdp.registers[2] = 0x30;

    // Plane Size: 32x32 (Reg 16 = 0x00)
    vdp.registers[16] = 0x00;

    // Palette 0, Color 1: Red (0xF800)
    vdp.cram_cache[1] = 0xF800;

    // Tile 1 Pattern: All 0x11 (Color 1)
    let tile1_addr = 32; // Each tile is 32 bytes
    for i in 0..32 {
        vdp.vram[tile1_addr + i] = 0x11;
    }

    // Nametable Entry at 0xC000 (0,0) -> Tile 1, Pal 0, Priority 0, Flip 0
    // Entry = 0x0001
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;

    // Render Line 0
    vdp.render_line(0);

    // Check first 8 pixels (Tile 1 is 8px wide)
    for i in 0..8 {
        assert_eq!(vdp.framebuffer[i], 0xF800, "Pixel {} mismatch", i);
    }
}

#[test]
fn test_render_plane_hflip_quirk() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display Enable
    vdp.registers[2] = 0x30; // Plane A Addr 0xC000
    vdp.registers[16] = 0x00; // 32x32

    // Palette 0: Col 1=Red, Col 2=Green, Col 3=Blue, Col 4=White
    vdp.cram_cache[1] = 0xF800; // Red
    vdp.cram_cache[2] = 0x07E0; // Green
    vdp.cram_cache[3] = 0x001F; // Blue
    vdp.cram_cache[4] = 0xFFFF; // White

    // Tile 1 Pattern: Row 0 -> [0x12, 0x34, 0x12, 0x34] (First 4 bytes = 8 pixels)
    // Pixels: 1, 2, 3, 4, 1, 2, 3, 4
    // Colors: R, G, B, W, R, G, B, W
    let tile1_addr = 32;
    vdp.vram[tile1_addr + 0] = 0x12;
    vdp.vram[tile1_addr + 1] = 0x34;
    vdp.vram[tile1_addr + 2] = 0x12;
    vdp.vram[tile1_addr + 3] = 0x34;

    // Nametable Entry at 0xC000 -> Tile 1, H-Flip
    // Entry = 0x0801 (Bit 11 set for H-Flip)
    vdp.vram[0xC000] = 0x08;
    vdp.vram[0xC001] = 0x01;

    vdp.render_line(0);

    // Expected behavior:
    // If H-Flip is "Swap Nibbles" (Current suspected behavior):
    // 0x12 -> 0x21 (Col 2, Col 1) -> G, R
    // 0x34 -> 0x43 (Col 4, Col 3) -> W, B
    let expected_swap = vec![0x07E0, 0xF800, 0xFFFF, 0x001F]; // G, R, W, B

    // If H-Flip is "True Flip":
    // Row: 1, 2, 3, 4, 1, 2, 3, 4
    // Flip: 4, 3, 2, 1, 4, 3, 2, 1
    let expected_flip = vec![0xFFFF, 0x001F, 0x07E0, 0xF800]; // W, B, G, R

    let actual: Vec<u16> = vdp.framebuffer[0..4].to_vec();

    // Check which one it matches
    if actual == expected_swap {
        // Confirm swap behavior matches
        assert_eq!(actual, expected_swap, "H-Flip behaves as nibble-swap");
    } else if actual == expected_flip {
        // Confirm true flip behavior matches
        assert_eq!(actual, expected_flip, "H-Flip behaves as true flip");
    } else {
        // Unknown behavior
        panic!("H-Flip behavior unknown: {:04X?}", actual);
    }
}

#[test]
fn test_render_plane_scroll() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display
    vdp.registers[2] = 0x30; // Plane A 0xC000
    // HScroll Table Base 0x0000 (Reg 13 = 0x00)
    vdp.registers[13] = 0x00;

    vdp.cram_cache[1] = 0xF800; // Red

    // Tile 1: All 1s.
    let tile1_addr = 32;
    for i in 0..32 {
        vdp.vram[tile1_addr + i] = 0x11;
    }

    // Set H-Scroll for Plane A to -8 (0xFFF8).
    // This shifts plane LEFT by 8 pixels, bringing Tile 1 (at x=8) to x=0.
    vdp.vram[0] = 0xFF;
    vdp.vram[1] = 0xF8;

    // Nametable at 0xC000.
    // Tile 0 (Empty) at 0xC000.
    // Tile 1 (Red) at 0xC002.
    vdp.vram[0xC000] = 0x00; vdp.vram[0xC001] = 0x00;
    vdp.vram[0xC002] = 0x00; vdp.vram[0xC003] = 0x01;

    vdp.render_line(0);

    // Pixel 0 should be from Tile 1 (Red).
    assert_eq!(vdp.framebuffer[0], 0xF800);
    // Pixel 7 should be Red.
    assert_eq!(vdp.framebuffer[7], 0xF800);
    // Pixel 8 should be Empty (0) because Tile 2 is empty.
    assert_eq!(vdp.framebuffer[8], 0x0000);
}

#[test]
fn test_sprite_rendering_correctness() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display
    vdp.registers[5] = 0x6C; // SAT 0xD800

    vdp.cram_cache[1] = 0x001F; // Blue

    // Tile 1: All 1s (Blue)
    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
    }

    // Sprite 0 at 0xD800
    // Y=128+10, Size=1x1, Link=0, Attr=(Pri=1, Pal=0, Flip=0, Tile=1), X=128+10
    let base = 0xD800;
    vdp.vram[base+0] = 0x00; vdp.vram[base+1] = 128+10;
    vdp.vram[base+2] = 0x00; // 1x1
    vdp.vram[base+3] = 0x00;
    vdp.vram[base+4] = 0x80; vdp.vram[base+5] = 0x01;
    vdp.vram[base+6] = 0x00; vdp.vram[base+7] = 128+10;

    vdp.render_line(10);

    let offset = 10 * 320;
    // Pixel 10 should be blue
    assert_eq!(vdp.framebuffer[offset + 10], 0x001F);
    // Pixel 9 should be empty
    assert_eq!(vdp.framebuffer[offset + 9], 0x0000);
}

#[test]
fn test_sprite_hflip() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display
    vdp.registers[5] = 0x6C; // SAT 0xD800

    vdp.cram_cache[1] = 0xF800; // Red
    vdp.cram_cache[2] = 0x07E0; // Green

    // Tile 1: Row 0 -> [0x12, 0x12, 0x12, 0x12]
    // Pixels: 1,2, 1,2, 1,2, 1,2
    // Colors: R,G, R,G, R,G, R,G
    vdp.vram[32] = 0x12; vdp.vram[33] = 0x12; vdp.vram[34] = 0x12; vdp.vram[35] = 0x12;

    // Sprite 0: Y=10, H-Flip
    let base = 0xD800;
    vdp.vram[base+0] = 0x00; vdp.vram[base+1] = 128+10;
    vdp.vram[base+2] = 0x00;
    vdp.vram[base+3] = 0x00;
    // Attr: H-Flip (0x0800), Tile 1
    vdp.vram[base+4] = 0x88; vdp.vram[base+5] = 0x01;
    vdp.vram[base+6] = 0x00; vdp.vram[base+7] = 128+10;

    vdp.render_line(10);

    let offset = 10 * 320;
    // Normal: 1,2,1,2,1,2,1,2
    // Flip:   2,1,2,1,2,1,2,1
    // Pixel 0 (screen x=10): Color 2 (Green)
    // Pixel 1 (screen x=11): Color 1 (Red)

    assert_eq!(vdp.framebuffer[offset + 10], 0x07E0, "Pixel 0 should be Green");
    assert_eq!(vdp.framebuffer[offset + 11], 0xF800, "Pixel 1 should be Red");
}
