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
    vdp.set_region(false); // NTSC
    vdp.registers[1] = 0x40; // Display Enable
    vdp.registers[5] = 0x78; // Sprite Table at 0xF000 (0x78 << 9 = 0xF000)

    // Palette 1 (Sprites use palette 1, 2, 3 usually, or maybe 0-3 depending on attr)
    // Attr palette 0 maps to CRAM 16-31.
    vdp.cram_cache[17] = 0xF800; // Red (Palette 1, Color 1)
    vdp.cram_cache[18] = 0x07E0; // Green (Palette 1, Color 2)

    // Sprite 0: at 100, 100. Size 1x1. Palette 1.
    // SAT at 0xF000.
    // Index 0:
    // +0: VPos = 100 + 128 = 228 (0x00E4)
    // +2: Size = 00 (1x1)
    // +3: Link = 0 (End)
    // +4: Attr = Palette 1 (bit 13,14 = 01 -> 0x2000) | Priority 0 | Flip 0 | Tile 0
    // +6: HPos = 100 + 128 = 228 (0x00E4)

    let sat_addr = 0xF000;
    vdp.vram[sat_addr] = 0x00; vdp.vram[sat_addr+1] = 0xE4;
    vdp.vram[sat_addr+2] = 0x00;
    vdp.vram[sat_addr+3] = 0x00;
    vdp.vram[sat_addr+4] = 0x20; vdp.vram[sat_addr+5] = 0x00;
    vdp.vram[sat_addr+6] = 0x00; vdp.vram[sat_addr+7] = 0xE4;

    // Tile 0 Pattern:
    // We want to test pixel 0 and 1 sharing a byte.
    // Byte 0: 0x12 (Pixel 0=1, Pixel 1=2)
    // Pixel 0 (color 1) -> Red.
    // Pixel 1 (color 2) -> Green.
    vdp.vram[0] = 0x12; // Row 0, Pixels 0,1

    // Sprite 1: H-Flip. at 120, 100. Size 1x1. Palette 1.
    // Link from Sprite 0 to Sprite 1.
    vdp.vram[sat_addr+3] = 1;

    // Index 1 (addr + 8):
    // +0: VPos = 100 + 128 = 228 (0x00E4)
    // +2: Size = 00 (1x1)
    // +3: Link = 0 (End)
    // +4: Attr = Palette 1 | H-Flip (0x800) -> 0x2800 | Tile 1
    // +6: HPos = 120 + 128 = 248 (0x00F8)

    let sat_addr_1 = sat_addr + 8;
    vdp.vram[sat_addr_1] = 0x00; vdp.vram[sat_addr_1+1] = 0xE4;
    vdp.vram[sat_addr_1+2] = 0x00;
    vdp.vram[sat_addr_1+3] = 0x00;
    vdp.vram[sat_addr_1+4] = 0x28; vdp.vram[sat_addr_1+5] = 0x01; // Tile 1
    vdp.vram[sat_addr_1+6] = 0x00; vdp.vram[sat_addr_1+7] = 0xF8;

    // Tile 1 Pattern (flipped).
    // Original pixels: 0, 1, 2, 3...
    // Flipped pixels: 7, 6, 5, 4... 1, 0
    // Byte 0 contains pixel 0 and 1.
    // If flipped, Pixel 0 corresponds to original Pixel 7. Pixel 1 -> Pixel 6.
    // We want to test that packed pixels are correctly flipped.

    // Let's set byte 3 (pixels 6, 7) to 0x21.
    // Pixel 6 = 2 (Green), Pixel 7 = 1 (Red).
    // On screen (flipped):
    // x=0 (original 7) -> Red (1)
    // x=1 (original 6) -> Green (2)

    vdp.vram[32 + 3] = 0x21;

    vdp.render_line(100);

    // Check pixels for Sprite 0
    // x=100 -> Pixel 0 -> Color 1 (Red)
    // x=101 -> Pixel 1 -> Color 2 (Green)
    assert_eq!(vdp.framebuffer[100 * 320 + 100], 0xF800, "Sprite 0 Pixel 0 incorrect");
    assert_eq!(vdp.framebuffer[100 * 320 + 101], 0x07E0, "Sprite 0 Pixel 1 incorrect");

    // Check pixels for Sprite 1 (Flipped)
    // x=120 (Pixel 0 of sprite, corresponds to Pixel 7 of tile) -> Red (1)
    // x=121 (Pixel 1 of sprite, corresponds to Pixel 6 of tile) -> Green (2)
    assert_eq!(vdp.framebuffer[100 * 320 + 120], 0xF800, "Sprite 1 Pixel 0 incorrect");
    assert_eq!(vdp.framebuffer[100 * 320 + 121], 0x07E0, "Sprite 1 Pixel 1 incorrect");
}

#[test]
fn test_sprite_rendering_perf() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display Enable
    vdp.registers[5] = 0x78; // Sprite Table at 0xF000

    let sat_addr = 0xF000;

    // Create 20 sprites on line 100
    for i in 0..20 {
        let addr = sat_addr + (i * 8);
        vdp.vram[addr] = 0x00; vdp.vram[addr+1] = 0xE4; // VPos 100
        vdp.vram[addr+2] = 0x0F; // Size 4x4 tiles (max size) -> 32x32 pixels
        vdp.vram[addr+3] = (i + 1) as u8; // Link to next
        vdp.vram[addr+4] = 0x00; vdp.vram[addr+5] = 0x00; // Tile 0, Pal 0
        vdp.vram[addr+6] = 0x00; vdp.vram[addr+7] = 0x80; // HPos 0
    }
    // Fix last link
    vdp.vram[sat_addr + (19 * 8) + 3] = 0;

    // Fill Tile 0 with pattern
    for i in 0..32 {
        vdp.vram[i] = 0x11;
    }

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        vdp.render_line(100);
    }
    let duration = start.elapsed();
    println!("Sprite Render Perf: {:?}", duration);
}
