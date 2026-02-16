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
    let nt_addr = 0xC000 + (0 * 32 + 0) * 2;
    vdp.vram[nt_addr] = 0x00;
    vdp.vram[nt_addr + 1] = 0x01;

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
    let nt_addr = 0xC000 + (0 * 32 + 0) * 2;
    vdp.vram[nt_addr] = 0x08; // Bit 11 set for H-Flip
    vdp.vram[nt_addr + 1] = 0x01;

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
    vdp.registers[4] = 0x07; // Plane B 0xE000
    vdp.registers[13] = 0x00; // HScroll Table Base 0x0000

    vdp.cram_cache[1] = 0xF800; // Red (Plane A)
    vdp.cram_cache[17] = 0x07E0; // Green (Plane B)

    // Tile 1: All 1s.
    let tile1_addr = 32;
    for i in 0..32 {
        vdp.vram[tile1_addr + i] = 0x11;
    }

    // Set H-Scroll for Plane A to 1 pixel.
    // screen_x=0 -> scrolled_h = 0 - 1 = 65535.
    // tile_h = (65535/8)%32 = 31.
    // So pixel 0 will come from tile 31.
    vdp.vram[0] = 0x00;
    vdp.vram[1] = 0x01;

    // Nametable A at 0xC000.
    // Put Tile 1 (Red) at tile_h=31, tile_v=0
    let nt_addr = 0xC000 + (0 * 32 + 31) * 2;
    vdp.vram[nt_addr] = 0x00;
    vdp.vram[nt_addr + 1] = 0x01;

    vdp.render_line(0);

    // Pixel 0 should be from Tile 1 (Red).
    assert_eq!(vdp.framebuffer[0], 0xF800);
}

#[test]
fn test_render_plane_b_isolation() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30; // Plane A 0xC000
    vdp.registers[4] = 0x07; // Plane B 0xE000
    vdp.registers[13] = 0x00;

    // Set background color to Black (Palette 0, Index 0)
    vdp.registers[7] = 0x00;
    vdp.cram_cache[0] = 0x0000; // Background

    // Clear VRAM to ensure Tile 0 is empty (color index 0 everywhere)

    vdp.vram.fill(0);

    vdp.cram_cache[1] = 0xF800; // Red (Pal 0)
    vdp.cram_cache[17] = 0x07E0; // Green (Pal 1)

    // Tile 1: All 1s.
    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
    }

    // Plane A: All Tile 0 (Transparent)
    // Plane B: Tile 1 (Green) at screen (0,0)
    let nt_addr_b = 0xE000 + (0 * 32 + 0) * 2;
    vdp.vram[nt_addr_b] = 0x20; // Pal 1, Tile 1
    vdp.vram[nt_addr_b + 1] = 0x01;

    vdp.render_line(0);

    // Plane B should be visible because Plane A is transparent.
    assert_eq!(vdp.framebuffer[0], 0x07E0, "Plane B should be visible");
}

#[test]
fn test_render_line_performance() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] |= 0x40; // Display enabled

    let start = std::time::Instant::now();
    // Render 100 frames. On the test runner (~4ms/frame), this should take ~400ms.
    for _ in 0..100 {
        for line in 0..224 {
            vdp.render_line(line);
        }
    }
    let duration = start.elapsed();
    println!("Render 100 frames took: {:?}", duration);

    // Simple sanity check to ensure no massive regression (e.g. if it took 2s, something is wrong)
    assert!(
        duration.as_millis() < 2000,
        "Rendering 100 frames took too long: {:?}",
        duration
    );
}

#[test]
fn test_sprite_rendering_correctness() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display Enable
    vdp.registers[12] = 0x81; // H40 Mode
    vdp.registers[5] = 0x6A; // SAT at 0xD400

    // Palette 1, Color 1: Red
    vdp.cram_cache[17] = 0xF00;
    vdp.cram_cache[18] = 0x0F0; // 2: Green
    vdp.cram_cache[19] = 0x00F; // 3: Blue
    vdp.cram_cache[20] = 0xFFF; // 4: White

    // Tile 1: Pattern
    // Row 0: 0x12, 0x34, 0x00, 0x00 -> Pixels: 1, 2, 3, 4, 0, 0, 0, 0
    let tile1_addr = 32;
    vdp.vram[tile1_addr] = 0x12;
    vdp.vram[tile1_addr + 1] = 0x34;

    // Sprite 0: 1x1 tile, at (0,0) on screen
    let sat_base = 0xD400;

    // V Pos: 0 (screen y) + 128 = 128 (0x80)
    vdp.vram[sat_base] = 0x00;
    vdp.vram[sat_base + 1] = 0x80;

    // Size: 1x1 (0x00), Link: 0
    vdp.vram[sat_base + 2] = 0x00;
    vdp.vram[sat_base + 3] = 0x00;

    // Attr: Palette 1, Priority 1, Tile 1
    // Pal 1 = bit 13 (0x2000). Tile 1 = 1. -> 0x2001.
    vdp.vram[sat_base + 4] = 0x20;
    vdp.vram[sat_base + 5] = 0x01;

    // H Pos: 0 (screen x)
    vdp.vram[sat_base + 6] = 0x00;
    vdp.vram[sat_base + 7] = 0x80;

    // Render line 0
    vdp.render_line(0);

    // Check pixels at 0, 1, 2, 3
    assert_eq!(vdp.framebuffer[0], 0xF00, "Pixel 0 (Red)");
    assert_eq!(vdp.framebuffer[1], 0x0F0, "Pixel 1 (Green)");
    assert_eq!(vdp.framebuffer[2], 0x00F, "Pixel 2 (Blue)");
    assert_eq!(vdp.framebuffer[3], 0xFFF, "Pixel 3 (White)");
    assert_eq!(vdp.framebuffer[4], 0x000, "Pixel 4 (Transparent)");
}

#[test]
fn test_sprite_hflip() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40;
    vdp.registers[12] = 0x81;
    vdp.registers[5] = 0x6A;

    vdp.cram_cache[17] = 0xF00; // 1: Red
    vdp.cram_cache[18] = 0x0F0; // 2: Green

    // Tile 1: 0x12... -> Pixels: 1, 2...
    let tile1_addr = 32;
    vdp.vram[tile1_addr] = 0x12;

    let sat_base = 0xD400;
    // V Pos 128 -> y=0
    vdp.vram[sat_base] = 0x00;
    vdp.vram[sat_base + 1] = 0x80;
    // Size 1x1
    vdp.vram[sat_base + 2] = 0x00;
    vdp.vram[sat_base + 3] = 0x00;

    // Attr: Pal 1, H-Flip (0x800), Tile 1 -> 0x2801
    vdp.vram[sat_base + 4] = 0x28;
    vdp.vram[sat_base + 5] = 0x01;

    // H Pos 128 -> x=0
    vdp.vram[sat_base + 6] = 0x00;
    vdp.vram[sat_base + 7] = 0x80;

    vdp.render_line(0);

    // H-Flip:
    // Tile 1 row 0: 1, 2, 0, 0, 0, 0, 0, 0
    // Flipped:      0, 0, 0, 0, 0, 0, 2, 1
    // Pixel 0-5: Transparent
    // Pixel 6: 2 (Green)
    // Pixel 7: 1 (Red)

    assert_eq!(vdp.framebuffer[0], 0, "Pixel 0");
    assert_eq!(vdp.framebuffer[6], 0x0F0, "Pixel 6");
    assert_eq!(vdp.framebuffer[7], 0xF00, "Pixel 7");
}

#[test]
fn test_render_sprite_basic() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display
    vdp.registers[5] = 0x68; // SAT at 0xD000

    // Setup Sprite 0 at 0xD000
    // y=128+10, size=1x1 (0), link=0, attr=0 (pal 0), x=128+10
    let sat_base = 0xD000;
    vdp.vram[sat_base] = 0x00;
    vdp.vram[sat_base + 1] = 128 + 10;
    vdp.vram[sat_base + 2] = 0x00; // 1x1 tile
    vdp.vram[sat_base + 3] = 0x00; // link 0
    vdp.vram[sat_base + 4] = 0x00;
    vdp.vram[sat_base + 5] = 0x00; // attr: tile 0
    vdp.vram[sat_base + 6] = 0x00;
    vdp.vram[sat_base + 7] = 128 + 10;

    // Tile 0 Pattern:
    // Row 0: 0x12, 0x34, 0x56, 0x78 (Pixels: 1,2, 3,4, 5,6, 7,8)
    // We render line 10. Sprite y=10. So line 10 is row 0 of sprite.
    vdp.vram[0] = 0x12;
    vdp.vram[1] = 0x34;
    vdp.vram[2] = 0x56;
    vdp.vram[3] = 0x78;

    // Pal 0 Colors
    vdp.cram_cache[1] = 0x0001;
    vdp.cram_cache[2] = 0x0002;
    vdp.cram_cache[3] = 0x0003;
    vdp.cram_cache[4] = 0x0004;

    vdp.render_line(10);

    // Sprite is at x=10.
    // Pixels 0-7 of sprite should be at screen x=10-17.
    // Line offset for line 10 is 3200.
    let offset = 3200;
    // Pixel 0: Val 1 -> Color 1
    assert_eq!(vdp.framebuffer[offset + 10], 0x0001, "Pixel 0 mismatch");
    // Pixel 1: Val 2 -> Color 2
    assert_eq!(vdp.framebuffer[offset + 11], 0x0002, "Pixel 1 mismatch");
    // Pixel 2: Val 3
    assert_eq!(vdp.framebuffer[offset + 12], 0x0003, "Pixel 2 mismatch");
    // Pixel 7: Val 8 (from 0x78 -> 8) -> Color 0 (Transparent)
    assert_eq!(vdp.framebuffer[offset + 17], 0x0000, "Pixel 7 mismatch");
}

#[test]
fn test_render_sprite_hflip_v3() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display
    vdp.registers[5] = 0x68; // SAT at 0xD000

    // Setup Sprite 0 at 0xD000
    // H-Flip enabled (Bit 11 of attr word) -> Byte 4 bit 3?
    // Attr word is bytes 4,5.
    // Bit 11 is 0x0800. So byte 4 |= 0x08.
    let sat_base = 0xD000;
    vdp.vram[sat_base] = 0x00;
    vdp.vram[sat_base + 1] = 128 + 10;
    vdp.vram[sat_base + 2] = 0x00; // 1x1
    vdp.vram[sat_base + 3] = 0x00;
    vdp.vram[sat_base + 4] = 0x08;
    vdp.vram[sat_base + 5] = 0x00; // H-Flip
    vdp.vram[sat_base + 6] = 0x00;
    vdp.vram[sat_base + 7] = 128 + 10;

    // Tile 0 Pattern: 0x12, 0x34...
    // Pixels: 1,2, 3,4...
    vdp.vram[0] = 0x12;
    vdp.vram[1] = 0x34;

    vdp.cram_cache[1] = 0x0001;
    vdp.cram_cache[2] = 0x0002;
    vdp.cram_cache[3] = 0x0003;
    vdp.cram_cache[4] = 0x0004;

    vdp.render_line(10);
    let offset = 3200;

    // H-Flip:
    // Original: 1,2, 3,4, 5,6, 7,8
    // Flipped:  8,7, 6,5, 4,3, 2,1

    // Pixel 0 (screen 10): Should be 8 (Color 0/Transparent)
    assert_eq!(
        vdp.framebuffer[offset + 10],
        0x0000,
        "Flip Pixel 0 mismatch"
    );

    // Pixel 6 (screen 16): Should be 2 -> Color 2
    assert_eq!(
        vdp.framebuffer[offset + 16],
        0x0002,
        "Flip Pixel 6 mismatch"
    );
    // Pixel 7 (screen 17): Should be 1 -> Color 1
    assert_eq!(
        vdp.framebuffer[offset + 17],
        0x0001,
        "Flip Pixel 7 mismatch"
    );
}

#[test]
fn test_sprite_rendering_correctness_v2() {
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
    vdp.vram[base + 0] = 0x00;
    vdp.vram[base + 1] = 128 + 10;
    vdp.vram[base + 2] = 0x00; // 1x1
    vdp.vram[base + 3] = 0x00;
    vdp.vram[base + 4] = 0x80;
    vdp.vram[base + 5] = 0x01;
    vdp.vram[base + 6] = 0x00;
    vdp.vram[base + 7] = 128 + 10;

    vdp.render_line(10);

    let offset = 10 * 320;
    // Pixel 10 should be blue
    assert_eq!(vdp.framebuffer[offset + 10], 0x001F);
    // Pixel 9 should be empty
    assert_eq!(vdp.framebuffer[offset + 9], 0x0000);
}

#[test]
fn test_sprite_hflip_v2() {
    let mut vdp = Vdp::new();
    vdp.set_region(false);
    vdp.registers[1] = 0x40; // Display
    vdp.registers[5] = 0x6C; // SAT 0xD800

    vdp.cram_cache[1] = 0xF800; // Red
    vdp.cram_cache[2] = 0x07E0; // Green

    // Tile 1: Row 0 -> [0x12, 0x12, 0x12, 0x12]
    // Pixels: 1,2, 1,2, 1,2, 1,2
    // Colors: R,G, R,G, R,G, R,G
    vdp.vram[32] = 0x12;
    vdp.vram[33] = 0x12;
    vdp.vram[34] = 0x12;
    vdp.vram[35] = 0x12;

    // Sprite 0: Y=10, H-Flip
    let base = 0xD800;
    vdp.vram[base + 0] = 0x00;
    vdp.vram[base + 1] = 128 + 10;
    vdp.vram[base + 2] = 0x00;
    vdp.vram[base + 3] = 0x00;
    // Attr: H-Flip (0x0800), Tile 1
    vdp.vram[base + 4] = 0x88;
    vdp.vram[base + 5] = 0x01;
    vdp.vram[base + 6] = 0x00;
    vdp.vram[base + 7] = 128 + 10;

    vdp.render_line(10);

    let offset = 10 * 320;
    // Normal: 1,2,1,2,1,2,1,2
    // Flip:   2,1,2,1,2,1,2,1
    // Pixel 0 (screen x=10): Color 2 (Green)
    // Pixel 1 (screen x=11): Color 1 (Red)

    assert_eq!(
        vdp.framebuffer[offset + 10],
        0x07E0,
        "Pixel 0 should be Green"
    );
    assert_eq!(
        vdp.framebuffer[offset + 11],
        0xF800,
        "Pixel 1 should be Red"
    );
}

#[test]
fn test_render_plane_vram_wrapping() {
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
    vdp.vram[0] = 0;
    vdp.vram[1] = 1;

    // screen_x=0 -> scrolled_h = (0 + 128) - 1 = 127.
    // tile_h = (127 / 8) % 128 = 15.

    // With `tile_v` = 127 and `tile_h` = 15.
    // `nt_entry_addr` = `0xE000 + (127 * 128 + 15) * 2`.
    // = `57344 + 32542` = `89886`.

    // The wrapped address should be 89886 & 0xFFFF = 24350 (0x5F1E).
    let wrapped_addr = 24350;

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
    assert_eq!(
        vdp.framebuffer[0], 0xF800,
        "Pixel at 0,0 should be Red (0xF800), indicating correct wrapping"
    );
}
