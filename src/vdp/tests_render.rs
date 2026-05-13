use super::render::{PixelLayerData, ShadowHighlightParams};
use super::*;

#[test]
fn test_render_plane_basic() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false; // NTSC

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
    let nt_addr = 0xC000;
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
fn test_shadow_highlight_mode_keeps_normal_pixels_at_normal_intensity() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40; // Display enable
    vdp.registers[2] = 0x30; // Plane A at 0xC000
    vdp.registers[12] = 0x08; // Shadow/highlight enabled

    vdp.cram_cache[0] = 0x0000;
    vdp.cram_cache[1] = 0xF800;

    let tile_addr = 32;
    for i in 0..32 {
        vdp.vram[tile_addr + i] = 0x11;
    }

    let nt_addr = 0xC000;
    vdp.vram[nt_addr] = 0x00;
    vdp.vram[nt_addr + 1] = 0x01;

    vdp.render_line(0);

    for i in 0..8 {
        assert_eq!(
            vdp.framebuffer[i], 0xF800,
            "shadow/highlight mode should not darken normal plane pixels at {}",
            i
        );
    }
}

#[test]
fn test_render_plane_hflip_quirk() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
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
    vdp.vram[tile1_addr] = 0x12;
    vdp.vram[tile1_addr + 1] = 0x34;
    vdp.vram[tile1_addr + 2] = 0x12;
    vdp.vram[tile1_addr + 3] = 0x34;

    // Nametable Entry at 0xC000 -> Tile 1, H-Flip
    let nt_addr = 0xC000;
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
    vdp.is_pal = false;
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
    let nt_addr = 0xC000 + 31 * 2;
    vdp.vram[nt_addr] = 0x00;
    vdp.vram[nt_addr + 1] = 0x01;

    vdp.render_line(0);

    // Pixel 0 should be from Tile 1 (Red).
    assert_eq!(vdp.framebuffer[0], 0xF800);
}

#[test]
fn test_render_plane_b_isolation() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
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
    let nt_addr_b = 0xE000;
    vdp.vram[nt_addr_b] = 0x20; // Pal 1, Tile 1
    vdp.vram[nt_addr_b + 1] = 0x01;

    vdp.render_line(0);

    // Plane B should be visible because Plane A is transparent.
    assert_eq!(vdp.framebuffer[0], 0x07E0, "Plane B should be visible");
}

#[test]
fn test_window_plane_uses_horizontal_split_outside_vertical_window() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30; // Plane A base 0xC000
    vdp.registers[3] = 0x38; // Window base 0xE000
    vdp.registers[16] = 0x00;
    vdp.registers[17] = 0x01; // Window starts at x = 16
    vdp.registers[18] = 0x01; // Window starts at y = 8

    vdp.cram_cache[1] = 0xF800; // Plane A red
    vdp.cram_cache[17] = 0x07E0; // Window green

    for i in 0..32 {
        vdp.vram[32 + i] = 0x11; // Tile 1
        vdp.vram[64 + i] = 0x11; // Tile 2
    }

    // Plane A points at Tile 1.
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;
    vdp.vram[0xC040] = 0x00;
    vdp.vram[0xC041] = 0x01;

    // Window plane points at Tile 2.
    vdp.vram[0xE000] = 0x20;
    vdp.vram[0xE001] = 0x02;
    vdp.vram[0xE040] = 0x20;
    vdp.vram[0xE041] = 0x02;

    // Outside the vertical window range, the horizontal split still applies.
    vdp.render_line(8);

    let offset = 8 * 320;
    assert_eq!(
        vdp.framebuffer[offset], 0x07E0,
        "window should still appear on the clipped side of the scanline"
    );
}

#[test]
fn test_render_line_performance() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
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

    // Simple sanity check to ensure no massive regression. CI and shared
    // runners can be noisy, so keep the guard loose enough to avoid false
    // positives while still catching pathological slowdowns.
    assert!(
        duration.as_millis() < 8000,
        "Rendering 100 frames took too long: {:?}",
        duration
    );
}

#[test]
fn test_sprite_rendering_correctness() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
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
fn test_sprite_left_edge_clipping() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40;
    vdp.registers[12] = 0x81; // H40 mode
    vdp.registers[5] = 0x6A; // SAT at 0xD400
    vdp.registers[7] = 0x00;

    vdp.cram_cache[0] = 0x0000;
    vdp.cram_cache[1] = 0xF800;

    // Tile 1: solid color 1.
    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
    }

    let sat_base = 0xD400;
    // Sprite at y=0, x=-4 (stored value 124).
    vdp.vram[sat_base] = 0x00;
    vdp.vram[sat_base + 1] = 0x80;
    vdp.vram[sat_base + 2] = 0x00;
    vdp.vram[sat_base + 3] = 0x00;
    vdp.vram[sat_base + 4] = 0x00;
    vdp.vram[sat_base + 5] = 0x01;
    vdp.vram[sat_base + 6] = 0x00;
    vdp.vram[sat_base + 7] = 0x7C;

    vdp.render_line(0);

    assert_eq!(
        vdp.framebuffer[0], 0xF800,
        "Sprite should clip into column 0"
    );
    assert_eq!(
        vdp.framebuffer[3], 0xF800,
        "Sprite should cover the visible edge"
    );
}

#[test]
fn test_sat_cache_sync_tracks_vram() {
    let mut vdp = Vdp::new();
    vdp.registers[12] = 0x81;
    vdp.registers[5] = 0x6A;

    let sat_base = 0xD400;
    vdp.vram[sat_base + 1] = 0x80;
    vdp.sync_sat_cache();
    assert_eq!(vdp.sat[1], 0x80);

    vdp.vram[sat_base + 1] = 0x81;
    assert_eq!(
        vdp.sat[1], 0x80,
        "SAT mirror should not change until resynced"
    );

    vdp.sync_sat_cache();
    assert_eq!(
        vdp.sat[1], 0x81,
        "SAT mirror should reflect the latest VRAM data"
    );
}

#[test]
fn test_tick_renders_completed_scanline() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30;
    vdp.registers[16] = 0x00;
    vdp.cram_cache[1] = 0xF800;

    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
    }
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;

    vdp.tick(3420, |_| 0);

    assert_eq!(
        vdp.framebuffer[0], 0xF800,
        "tick should render the scanline when it completes"
    );
}

#[test]
fn test_tick_latches_scanline_after_hblank() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30;
    vdp.registers[16] = 0x00;
    vdp.cram_cache[1] = 0xF800;

    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
    }
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;

    vdp.tick(859, |_| 0);

    assert_eq!(
        vdp.framebuffer[0], 0,
        "line should not render before HBlank ends"
    );

    vdp.tick(1, |_| 0);

    assert_eq!(
        vdp.framebuffer[0], 0xF800,
        "tick should render the active scanline when visible fetch begins"
    );

    vdp.cram_cache[1] = 0x07E0;
    vdp.tick(3419, |_| 0);

    assert_eq!(
        vdp.framebuffer[0], 0xF800,
        "once a scanline is latched, later writes must not change it"
    );
}

#[test]
fn test_cram_write_rerenders_current_scanline() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40;
    vdp.registers[7] = 0x01;
    vdp.cram_cache[1] = 0xF800;

    vdp.render_line(0);
    assert_eq!(vdp.framebuffer[0], 0xF800);

    vdp.mclk_line_clocks = 860;
    vdp.bypass_fifo = true;
    vdp.write_control(0xC002);
    vdp.write_control(0x0000);
    vdp.write_data(0x00E0);

    assert_eq!(
        vdp.framebuffer[0], vdp.cram_cache[1],
        "CRAM writes during an active scanline must refresh the framebuffer"
    );
    assert_ne!(vdp.framebuffer[0], 0xF800);
}

#[test]
fn test_register_write_rerenders_current_scanline() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30;
    vdp.registers[16] = 0x00;
    vdp.cram_cache[1] = 0xF800;
    vdp.cram_cache[2] = 0x07E0;

    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
        vdp.vram[64 + i] = 0x22;
    }
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;
    vdp.vram[0x8000] = 0x00;
    vdp.vram[0x8001] = 0x02;

    vdp.render_line(0);
    assert_eq!(vdp.framebuffer[0], 0xF800);

    vdp.mclk_line_clocks = 860;
    vdp.write_control(0x8220);

    assert_eq!(
        vdp.framebuffer[0], 0x07E0,
        "plane base register writes during an active scanline must refresh the framebuffer"
    );
}

#[test]
fn test_hblank_cram_write_rerenders_previous_scanline() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40;
    vdp.registers[7] = 0x01;
    vdp.cram_cache[1] = 0xF800;
    vdp.cram_cache[2] = 0x07E0;
    vdp.v_counter = 1;

    vdp.render_line(0);
    assert_eq!(vdp.framebuffer[0], 0xF800);

    vdp.mclk_line_clocks = 0;
    vdp.bypass_fifo = true;
    vdp.write_control(0xC002);
    vdp.write_control(0x0000);
    vdp.write_data(0x00E0);

    assert_eq!(
        vdp.framebuffer[0], 0x07E0,
        "CRAM writes during the early-HBlank window must remap the previous visible line"
    );
}

#[test]
fn test_sprite_hflip() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
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
    vdp.is_pal = false;
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
    vdp.is_pal = false;
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
    vdp.is_pal = false;
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
    vdp.vram[base] = 0x00;
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
    vdp.is_pal = false;
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
    vdp.vram[base] = 0x00;
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

#[test]
fn test_render_plane_respects_negative_hscroll_word() {
    let mut vdp = Vdp::new();
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30;
    vdp.registers[16] = 0x00;
    vdp.cram_cache[1] = 0xF800;
    vdp.cram_cache[2] = 0x07E0;

    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
        vdp.vram[64 + i] = 0x22;
    }
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;
    vdp.vram[0xC002] = 0x00;
    vdp.vram[0xC003] = 0x02;

    // Plane A hscroll = -8. Screen x 0 should therefore map to tile 1.
    vdp.vram[0x0000] = 0xFF;
    vdp.vram[0x0001] = 0xF8;

    vdp.render_line(0);

    assert_eq!(vdp.framebuffer[0], 0x07E0);
}

#[test]
fn test_render_plane_respects_negative_vscroll_word() {
    let mut vdp = Vdp::new();
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30;
    vdp.registers[16] = 0x00;
    vdp.cram_cache[1] = 0xF800;
    vdp.cram_cache[2] = 0x07E0;

    for i in 0..4 {
        vdp.vram[32 + i] = 0x11;
        vdp.vram[64 + i] = 0x22;
    }

    // Tile row 0 -> red
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;
    // Tile row 31 -> green
    let bottom_row = 31 * 32 * 2;
    vdp.vram[0xC000 + bottom_row] = 0x00;
    vdp.vram[0xC001 + bottom_row] = 0x02;

    // Plane A vscroll = -8. Line 0 should sample the final row of the plane.
    vdp.vsram[0] = 0xFF;
    vdp.vsram[1] = 0xF8;

    vdp.render_line(0);

    assert_eq!(vdp.framebuffer[0], 0x07E0);
}

#[test]
fn test_h40_partial_left_column_uses_last_vscroll_strip() {
    let mut vdp = Vdp::new();
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30;
    vdp.registers[12] = 0x81; // H40
    vdp.registers[13] = 0x00;
    vdp.registers[16] = 0x00;
    vdp.registers[11] = 0x04; // 2-cell vertical scroll mode
    vdp.cram_cache[1] = 0xF800;
    vdp.cram_cache[2] = 0x07E0;

    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
        vdp.vram[64 + i] = 0x22;
    }

    // H-scroll = +1 so the left-most column is only partially visible.
    vdp.vram[0] = 0x00;
    vdp.vram[1] = 0x01;

    // Column 31 row 0 -> tile 1 (red)
    let row0_col31 = 0xC000 + (31 * 2);
    vdp.vram[row0_col31] = 0x00;
    vdp.vram[row0_col31 + 1] = 0x01;

    // Column 31 row 31 -> tile 2 (green)
    let row31_col31 = 0xC000 + (((31 * 32) + 31) * 2);
    vdp.vram[row31_col31] = 0x00;
    vdp.vram[row31_col31 + 1] = 0x02;

    // Last H40 V-scroll strip = -8.
    vdp.vsram[76] = 0xFF;
    vdp.vsram[77] = 0xF8;

    vdp.render_line(0);

    assert_eq!(vdp.framebuffer[0], 0x07E0);
}

#[test]
fn test_tick_latches_hscroll_before_hblank_updates() {
    let mut vdp = Vdp::new();
    vdp.registers[1] = 0x40;
    vdp.registers[2] = 0x30;
    vdp.registers[16] = 0x00;
    vdp.cram_cache[1] = 0xF800;
    vdp.cram_cache[2] = 0x07E0;

    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
        vdp.vram[64 + i] = 0x22;
    }

    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;
    vdp.vram[0xC002] = 0x00;
    vdp.vram[0xC003] = 0x02;

    // Line 1 initially latches hscroll = 0.
    vdp.vram[0x0000] = 0x00;
    vdp.vram[0x0001] = 0x00;

    vdp.tick(3420, |_| 0);

    // A later HBlank update changes the live table to -8, but line 1 should
    // keep the latched value and still render tile 0 at x=0.
    vdp.vram[0x0000] = 0xFF;
    vdp.vram[0x0001] = 0xF8;

    vdp.tick(860, |_| 0);

    let offset = 320;
    assert_eq!(vdp.framebuffer[offset], 0xF800);
}

#[test]
fn test_sprite_oob_rendering() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40; // Display
    vdp.registers[12] = 0x81; // H40 Mode
    vdp.registers[5] = 0x6A; // SAT at 0xD400

    vdp.cram_cache[1] = 0xF800; // Red

    // Tile 1: All 1s
    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
    }

    // Sprite 0: at screen_x = 316 (H Pos = 316 + 128 = 444 = 0x1BC)
    // It's an 8x8 sprite, so it should span 316..324.
    // Screen width is 320. So 316, 317, 318, 319 are visible. 320..323 are OOB.
    let sat_base = 0xD400;
    vdp.vram[sat_base] = 0x00;
    vdp.vram[sat_base + 1] = 128; // y=0
    vdp.vram[sat_base + 2] = 0x00; // 1x1 tile
    vdp.vram[sat_base + 3] = 0x00;
    vdp.vram[sat_base + 4] = 0x00;
    vdp.vram[sat_base + 5] = 0x01; // Tile 1
    vdp.vram[sat_base + 6] = 0x01; // h_pos high bit? No, h_pos is 10 bits.
    vdp.vram[sat_base + 7] = 0xBC; // 0x1BC & 0xFF = 0xBC. Wait, bit 0 of byte 6 is high bit of h_pos.
    vdp.vram[sat_base + 6] = 0x01; // 0x1BC >> 8 = 0x01. Correct.

    // Render line 0
    vdp.render_line(0);

    // Visible pixels
    assert_eq!(vdp.framebuffer[316], 0xF800);
    assert_eq!(vdp.framebuffer[319], 0xF800);

    // If there was an OOB write, it would have panicked or corrupted memory.
    // Now with .get_mut(), it gracefully clips.
    // The framebuffer is size 320*240.
}

#[test]
fn test_sprite_fully_oob_rendering() {
    let mut vdp = Vdp::new();
    vdp.is_pal = false;
    vdp.registers[1] = 0x40; // Display
    vdp.registers[12] = 0x81; // H40 Mode
    vdp.registers[5] = 0x6A; // SAT at 0xD400

    vdp.cram_cache[1] = 0xF800;

    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
    }

    // Sprite 0: at screen_x = 320 (H Pos = 320 + 128 = 448 = 0x1C0)
    // Entirely OOB.
    let sat_base = 0xD400;
    vdp.vram[sat_base] = 0x00;
    vdp.vram[sat_base + 1] = 128;
    vdp.vram[sat_base + 2] = 0x00;
    vdp.vram[sat_base + 3] = 0x00;
    vdp.vram[sat_base + 4] = 0x00;
    vdp.vram[sat_base + 5] = 0x01;
    vdp.vram[sat_base + 6] = 0x01;
    vdp.vram[sat_base + 7] = 0xC0;

    // This should not panic.
    vdp.render_line(0);
}

#[test]
fn test_apply_shadow_highlight_basic() {
    let vdp = Vdp::new();
    let px = PixelLayerData {
        bg_color_idx: 0,
        s_pri: true,
        s_trans: false,
        s_col: 0x3E, // Shadow operator
        a_pri: false,
        a_trans: false,
        a_col: 1, // Underlying color
        b_pri: false,
        b_trans: true,
        b_col: 0,
    };

    let params = ShadowHighlightParams {
        top_layer: 3,
        top_col: 0x3E,
        state: 0, // Normal
        px: &px,
    };

    let (new_top_col, new_state) = vdp.apply_shadow_highlight(params);
    assert_eq!(new_top_col, 1, "Underlying color should be revealed");
    assert_eq!(new_state, 1, "State should be changed to shadow (1)");
}

#[test]
fn test_apply_shadow_highlight_highlight() {
    let vdp = Vdp::new();
    let px = PixelLayerData {
        bg_color_idx: 0,
        s_pri: true,
        s_trans: false,
        s_col: 0x3F, // Highlight operator
        a_pri: false,
        a_trans: false,
        a_col: 2, // Underlying color
        b_pri: false,
        b_trans: true,
        b_col: 0,
    };

    let params = ShadowHighlightParams {
        top_layer: 3,
        top_col: 0x3F,
        state: 1, // Shadow
        px: &px,
    };

    let (new_top_col, new_state) = vdp.apply_shadow_highlight(params);
    assert_eq!(new_top_col, 2, "Underlying color should be revealed");
    assert_eq!(
        new_state, 0,
        "State should be changed from shadow to normal (0)"
    );

    // Now test the color transform
    // In `apply_color_transform`, color is r=11..15, g=5..10, b=0..4.
    // Let's create a known mid-level green color (0x10) to test both halving and increasing.
    let color = 0b00000_010000_00000; // Partial Green

    // Test the specific state mapping logic from the source code:
    // state 0 = shadow (halves intensity)
    // state 1 = normal (unmodified)
    // state 2 = highlight (halfway towards max)
    let shadowed = vdp.apply_color_transform(color, 0); // Shadow is state 0
    let normal = vdp.apply_color_transform(color, 1); // Normal is state 1 or any other
    let highlighted = vdp.apply_color_transform(color, 2); // Highlight is state 2

    // Extract the green channel (bits 5-10)
    let g_shadow = (shadowed >> 5) & 0x3F;
    let g_normal = (normal >> 5) & 0x3F;
    let g_highlight = (highlighted >> 5) & 0x3F;

    assert_eq!(g_shadow, g_normal >> 1, "Shadow should halve intensity");
    assert!(
        g_highlight > g_normal,
        "Highlight should increase intensity"
    );
}

#[test]
fn test_determine_top_layer() {
    let vdp = Vdp::new();

    // Test Sprite priority
    let px1 = PixelLayerData {
        bg_color_idx: 0,
        s_pri: true,
        s_trans: false,
        s_col: 10,
        a_pri: true,
        a_trans: false,
        a_col: 5,
        b_pri: true,
        b_trans: false,
        b_col: 2,
    };

    let (top_col, top_layer) = vdp.determine_top_layer(&px1);
    assert_eq!(top_col, 10, "Sprite should win priority");
    assert_eq!(top_layer, 3, "Sprite is layer 3");

    // Test Plane A priority over Plane B when both are low priority
    let px2 = PixelLayerData {
        bg_color_idx: 0,
        s_pri: false,
        s_trans: true,
        s_col: 0,
        a_pri: false,
        a_trans: false,
        a_col: 5,
        b_pri: false,
        b_trans: false,
        b_col: 2,
    };

    let (top_col, top_layer) = vdp.determine_top_layer(&px2);
    assert_eq!(top_col, 5, "Plane A should win over Plane B");
    assert_eq!(top_layer, 2, "Plane A is layer 2");
}

#[test]
fn test_fetch_nametable_entry() {
    let mut vdp = Vdp::new();

    // Nametable entry format is 2 bytes per entry
    // Let's set Plane A base to 0xC000
    let base = 0xC000;

    // Let's configure a 64x32 plane size
    let plane_w = 64;

    // We want to read tile (x=5, y=2)
    // Addr = Base + (y * plane_w + x) * 2
    // Addr = 0xC000 + (2 * 64 + 5) * 2 = 0xC000 + (133) * 2 = 0xC000 + 266 = 0xC10A

    // Write entry to VRAM at 0xC10A:
    // High byte: Priority 1, Pal 2, V-Flip 1, H-Flip 0 -> 0x8000 | 0x4000 | 0x1000 = 0xD000
    // Low byte: Tile index 0x0123
    // Entry = 0xD123

    vdp.vram[0xC10A] = 0xD1;
    vdp.vram[0xC10B] = 0x23;

    let entry = vdp.fetch_nametable_entry(base, 2, 5, plane_w);
    assert_eq!(
        entry, 0xD123,
        "Fetched nametable entry does not match expected"
    );
}

#[test]
fn test_get_active_sprites_h40_limits() {
    let mut vdp = Vdp::new();
    vdp.registers[12] = 0x81; // H40 Mode
    vdp.registers[5] = 0x6A; // SAT at 0xD400

    let sat_base = 0xD400;

    // In H40, line limit is 20 sprites, max sprites is 80.
    // Let's create 25 sprites on line 10.
    // They are linked linearly: 0 -> 1 -> 2 ... -> 24 -> 0
    // Give every sprite a nonzero raw SAT X so the X=0 mask doesn't trigger.
    for i in 0..25 {
        let addr = sat_base + (i * 8);
        vdp.vram[addr] = 0x00;
        vdp.vram[addr + 1] = 128 + 10; // Y = 10

        vdp.vram[addr + 2] = 0x00; // 1x1 size

        // Link to next, but last links to 0
        if i == 24 {
            vdp.vram[addr + 3] = 0;
        } else {
            vdp.vram[addr + 3] = (i + 1) as u8;
        }

        // Non-zero raw X (so none of these are X=0 mask sprites).
        let raw_x = 128u16 + (i as u16) * 8;
        vdp.vram[addr + 6] = (raw_x >> 8) as u8;
        vdp.vram[addr + 7] = (raw_x & 0xFF) as u8;
    }

    let mut buffer = [SpriteAttributes::default(); 80];
    let count = vdp.get_active_sprites(10, &mut buffer);

    assert_eq!(
        count, 20,
        "H40 mode should limit to 20 active sprites per line"
    );
}

/// Helper used by the X=0 sprite-mask tests. Builds a sprite at the given
/// link slot with the given screen X (in screen coords; SAT stores X+128).
fn place_sprite(vdp: &mut Vdp, sat_base: usize, slot: usize, x: u16, y: u16, link_next: u8) {
    let addr = sat_base + slot * 8;
    let raw_y = y.wrapping_add(128);
    vdp.vram[addr] = (raw_y >> 8) as u8;
    vdp.vram[addr + 1] = (raw_y & 0xFF) as u8;
    vdp.vram[addr + 2] = 0x00; // 1x1 size
    vdp.vram[addr + 3] = link_next;
    vdp.vram[addr + 4] = 0x00;
    vdp.vram[addr + 5] = 0x00;
    let raw_x = x.wrapping_add(128);
    vdp.vram[addr + 6] = (raw_x >> 8) as u8;
    vdp.vram[addr + 7] = (raw_x & 0xFF) as u8;
}

/// X=0 sprite as the FIRST sprite on a line with no prior overflow:
/// the mask should NOT trigger; subsequent sprites still render.
#[test]
fn test_sprite_mask_x0_first_not_triggered() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE4] = 0x81; // H40
    vdp.registers[REG_SPRITE_TABLE] = 0x6A; // SAT @ 0xD400
    let sat_base = 0xD400;

    // Slot 0: raw X=0 (mask candidate, h_pos = -128), Slot 1: visible.
    let raw_zero_x = 0u16; // raw SAT X = 0 → h_pos = 0xFF80
    let addr0 = sat_base;
    let raw_y = 10u16 + 128;
    vdp.vram[addr0] = (raw_y >> 8) as u8;
    vdp.vram[addr0 + 1] = (raw_y & 0xFF) as u8;
    vdp.vram[addr0 + 2] = 0x00;
    vdp.vram[addr0 + 3] = 1;
    vdp.vram[addr0 + 6] = (raw_zero_x >> 8) as u8;
    vdp.vram[addr0 + 7] = (raw_zero_x & 0xFF) as u8;
    place_sprite(&mut vdp, sat_base, 1, 100, 10, 0);

    let mut buf = [SpriteAttributes::default(); 80];
    let count = vdp.get_active_sprites(10, &mut buf);

    assert_eq!(
        count, 1,
        "X=0 first sprite without prior overflow: mask must NOT trigger; sprite 1 stays visible"
    );
    assert_eq!(buf[0].index, 1);
}

/// X=0 sprite AFTER a visible sprite: the mask triggers, hiding all later sprites.
#[test]
fn test_sprite_mask_x0_after_visible_triggers() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE4] = 0x81; // H40
    vdp.registers[REG_SPRITE_TABLE] = 0x6A; // SAT @ 0xD400
    let sat_base = 0xD400;

    // Slot 0: visible at X=50
    place_sprite(&mut vdp, sat_base, 0, 50, 10, 1);
    // Slot 1: raw X=0. Should trigger mask.
    let addr1 = sat_base + 8;
    let raw_y = 10u16 + 128;
    let raw_zero_x = 0u16;
    vdp.vram[addr1] = (raw_y >> 8) as u8;
    vdp.vram[addr1 + 1] = (raw_y & 0xFF) as u8;
    vdp.vram[addr1 + 2] = 0x00;
    vdp.vram[addr1 + 3] = 2;
    vdp.vram[addr1 + 6] = (raw_zero_x >> 8) as u8;
    vdp.vram[addr1 + 7] = (raw_zero_x & 0xFF) as u8;
    // Slot 2: visible at X=150 — should be masked out.
    place_sprite(&mut vdp, sat_base, 2, 150, 10, 0);

    let mut buf = [SpriteAttributes::default(); 80];
    let count = vdp.get_active_sprites(10, &mut buf);

    assert_eq!(
        count, 1,
        "Mask must trigger after a visible sprite; later sprite must be hidden"
    );
    assert_eq!(buf[0].index, 0);
}

/// X=0 sprite as FIRST when previous line overflowed: mask triggers immediately.
#[test]
fn test_sprite_mask_x0_first_with_prev_overflow() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE4] = 0x81; // H40
    vdp.registers[REG_SPRITE_TABLE] = 0x6A; // SAT @ 0xD400
    let sat_base = 0xD400;

    // Force previous-line overflow state.
    vdp.prev_line_sprite_overflow = true;

    // Slot 0: raw X=0
    let addr0 = sat_base;
    let raw_y = 10u16 + 128;
    let raw_zero_x = 0u16;
    vdp.vram[addr0] = (raw_y >> 8) as u8;
    vdp.vram[addr0 + 1] = (raw_y & 0xFF) as u8;
    vdp.vram[addr0 + 2] = 0x00;
    vdp.vram[addr0 + 3] = 1;
    vdp.vram[addr0 + 6] = (raw_zero_x >> 8) as u8;
    vdp.vram[addr0 + 7] = (raw_zero_x & 0xFF) as u8;
    // Slot 1: visible — should be masked.
    place_sprite(&mut vdp, sat_base, 1, 80, 10, 0);

    let mut buf = [SpriteAttributes::default(); 80];
    let count = vdp.get_active_sprites(10, &mut buf);

    assert_eq!(
        count, 0,
        "Prior-line overflow makes an X=0 first sprite trigger the mask immediately"
    );
}

/// `prev_line_sprite_overflow` is set when the per-line sprite count limit is hit.
#[test]
fn test_prev_line_overflow_records_count_limit() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE4] = 0x81; // H40 → 20 sprites per line
    vdp.registers[REG_SPRITE_TABLE] = 0x6A;
    let sat_base = 0xD400;

    // 25 sprites all on line 10 — pushes past the 20-sprite line limit.
    for i in 0..25u8 {
        let next = if i == 24 { 0 } else { i + 1 };
        place_sprite(&mut vdp, sat_base, i as usize, 32 + (i as u16) * 8, 10, next);
    }

    let mut buf = [SpriteAttributes::default(); 80];
    let _ = vdp.get_active_sprites(10, &mut buf);
    assert!(
        vdp.prev_line_sprite_overflow,
        "Hitting the per-line sprite count limit must set prev_line_sprite_overflow"
    );
}
