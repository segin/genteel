use super::*;

// Helper to write CRAM
fn write_cram(vdp: &mut Vdp, addr: u16, data: u16) {
    let cmd = 0xC000 | (addr & 0x3FFF);
    vdp.write_control(cmd);
    vdp.write_control(0x0000);
    vdp.write_data(data);
}

// Helper to write VRAM
fn write_vram(vdp: &mut Vdp, addr: u16, data: &[u8]) {
    let cmd = 0x4000 | (addr & 0x3FFF);
    let high = (addr >> 14) & 0x3;
    vdp.write_control(cmd);
    vdp.write_control(high);

    vdp.registers[15] = 2;

    for chunk in data.chunks(2) {
        let val = if chunk.len() == 2 {
            ((chunk[0] as u16) << 8) | (chunk[1] as u16)
        } else {
            (chunk[0] as u16) << 8
        };
        vdp.write_data(val);
    }
}

#[test]
fn test_render_background_color() {
    let mut vdp = Vdp::new();
    vdp.set_region(false); // NTSC

    // Reg 7: Background Color (Palette 0, Color 1)
    vdp.registers[7] = 0x01;

    // Write Red to Palette 0, Color 1
    write_cram(&mut vdp, 2, 0x000E);

    vdp.render_line(0);

    // RGB565 Red: 0xF800
    assert_eq!(vdp.framebuffer[0], 0xF800, "Background pixel should be Red");
    // Default screen width is 256
    assert_eq!(vdp.framebuffer[255], 0xF800, "Last pixel should be Red");
}

#[test]
fn test_render_display_disabled() {
    let mut vdp = Vdp::new();

    // Set background color to Blue
    vdp.registers[7] = 0x02; // Palette 0, Color 2
    write_cram(&mut vdp, 4, 0x0E00); // Blue -> 0x001F

    // Disable display (Reg 1 bit 6 = 0)
    vdp.registers[1] &= !0x40;

    // Setup Plane A with something that would render if enabled (e.g. Red)
    // Point Plane A to 0xC000
    vdp.registers[2] = 0x30;

    // Write Red Color (Pal 0 Col 1)
    write_cram(&mut vdp, 2, 0x000E);

    // Write Tile 1 Pattern (Solid Color 1)
    let tile_data = [0x11; 32];
    write_vram(&mut vdp, 0x0020, &tile_data);

    // Write Nametable Entry at 0xC000 -> Tile 1
    write_vram(&mut vdp, 0xC000, &[0x00, 0x01]);

    vdp.render_line(0);

    // Should see Background Blue (0x001F), NOT Plane A Red (0xF800)
    assert_eq!(vdp.framebuffer[0], 0x001F, "Should render background color when display disabled");
}

#[test]
fn test_render_plane_a_basic() {
    let mut vdp = Vdp::new();

    // Enable Display
    vdp.registers[1] |= 0x40;

    // Setup Palette 0 Color 1: Green -> 0x07E0
    write_cram(&mut vdp, 2, 0x00E0);

    // Setup Plane A Address: 0xC000
    vdp.registers[2] = 0x30;

    // Setup Plane Size: 32x32
    vdp.registers[16] = 0x00;

    // Write Tile 1 Pattern to VRAM 0x0020 (Solid Color 1)
    let tile_data = [0x11; 32];
    write_vram(&mut vdp, 0x0020, &tile_data);

    // Write Nametable Entry at 0xC000 -> Tile 1
    write_vram(&mut vdp, 0xC000, &[0x00, 0x01]);

    vdp.render_line(0);

    // Pixel 0 should be Green
    assert_eq!(vdp.framebuffer[0], 0x07E0, "Pixel 0 should be Green from Plane A");
    // Pixel 7 should be Green
    assert_eq!(vdp.framebuffer[7], 0x07E0, "Pixel 7 should be Green from Plane A");
    // Pixel 8 should be Background (Black/Transparent)
    assert_eq!(vdp.framebuffer[8], 0x0000, "Pixel 8 should be Black (Background)");
}

#[test]
fn test_render_plane_scroll() {
    let mut vdp = Vdp::new();
    vdp.registers[1] |= 0x40;

    // Palette 0, Color 1 = Red
    write_cram(&mut vdp, 2, 0x000E);

    // Plane A at 0xC000
    vdp.registers[2] = 0x30;

    // HScroll Table at 0xFC00
    vdp.registers[13] = 0x3F;

    // Tile 1: Solid Color 1
    let tile_data = [0x11; 32];
    write_vram(&mut vdp, 0x0020, &tile_data);

    // Nametable: Tile 1 at (0,0) and (0,1)
    write_vram(&mut vdp, 0xC000, &[0x00, 0x01, 0x00, 0x01]);

    // Set HScroll to 4 pixels
    write_vram(&mut vdp, 0xFC00, &[0x00, 0x04]);

    vdp.render_line(0);

    assert_eq!(vdp.framebuffer[0], 0x0000, "Pixel 0 should be Black (scrolled)");
    assert_eq!(vdp.framebuffer[4], 0xF800, "Pixel 4 should be Red (scrolled)");
}

#[test]
fn test_render_sprite_basic() {
    let mut vdp = Vdp::new();
    vdp.registers[1] |= 0x40;

    // Palette 0 Color 2 = Blue
    write_cram(&mut vdp, 4, 0x0E00);

    // SAT at 0xD800
    vdp.registers[5] = 0x6C;

    // Tile 1: Solid Color 2
    let tile_data = [0x22; 32];
    write_vram(&mut vdp, 0x0020, &tile_data);

    // Sprite 0 at (32, 32)
    let sprite_attr = [
        0x00, 0xA0, // VPos 160
        0x00, // Size 1x1
        0x00, // Link 0
        0x00, 0x01, // Attr: Tile 1
        0x00, 0xA0, // HPos 160
    ];
    write_vram(&mut vdp, 0xD800, &sprite_attr);

    vdp.render_line(32);

    assert_eq!(vdp.framebuffer[32 * 320 + 32], 0x001F, "Pixel 32 should be Blue from Sprite");
}

#[test]
fn test_render_priority() {
    let mut vdp = Vdp::new();
    vdp.registers[1] |= 0x40;

    // Setup Colors
    write_cram(&mut vdp, 2, 0x000E); // Red (Col 1)
    write_cram(&mut vdp, 4, 0x00E0); // Green (Col 2)
    write_cram(&mut vdp, 6, 0x0E00); // Blue (Col 3)

    // Addresses
    vdp.registers[4] = 0x07; // Plane B 0xE000
    vdp.registers[2] = 0x30; // Plane A 0xC000
    vdp.registers[5] = 0x6C; // SAT 0xD800

    // Tiles
    write_vram(&mut vdp, 0x0020, &[0x11; 32]); // Tile 1 Red
    write_vram(&mut vdp, 0x0040, &[0x22; 32]); // Tile 2 Green
    write_vram(&mut vdp, 0x0060, &[0x33; 32]); // Tile 3 Blue

    // Scenario 1: Plane A (Low) vs Plane B (Low). Plane A should win.
    write_vram(&mut vdp, 0xE000, &[0x00, 0x01]); // Plane B -> Tile 1 Red
    write_vram(&mut vdp, 0xC000, &[0x00, 0x02]); // Plane A -> Tile 2 Green

    vdp.render_line(0);
    assert_eq!(vdp.framebuffer[0], 0x07E0, "Plane A Low should overwrite Plane B Low");

    // Scenario 2: Sprite (Low) vs Plane A (Low). Sprite should win.
    // Place Sprite (Tile 3 Blue) at (0,0)
    let sprite_attr = [
        0x00, 0x80, // VPos 128
        0x00, 0x00, 0x00, 0x03, // Tile 3
        0x00, 0x80, // HPos 128
    ];
    write_vram(&mut vdp, 0xD800, &sprite_attr);

    vdp.render_line(0);
    assert_eq!(vdp.framebuffer[0], 0x001F, "Sprite Low should overwrite Plane A Low");

    // Scenario 3: Plane B High vs Sprite Low. Plane B High should win.
    // Set Plane B entry to Priority (Bit 15).
    write_vram(&mut vdp, 0xE000, &[0x80, 0x01]); // Priority + Tile 1 Red

    vdp.render_line(0);
    assert_eq!(vdp.framebuffer[0], 0xF800, "Plane B High should overwrite Sprite Low");
}
