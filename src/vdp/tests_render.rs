use super::*;

// Helper to write a word to VRAM at a specific address
#[allow(dead_code)]
fn write_vram(vdp: &mut Vdp, addr: u16, data: &[u16]) {
    // Set auto-increment to 2
    vdp.registers[15] = 2;

    // Set address for write
    let cmd = 0x4000 | (addr & 0x3FFF);
    vdp.write_control(cmd);
    let cmd2 = ((addr >> 14) & 0x03) as u16; // CD high is 0 for VRAM write
    vdp.write_control(cmd2); // control_code should become 1

    for &val in data {
        vdp.write_data(val);
    }
}

// Helper to write color to CRAM
fn write_cram(vdp: &mut Vdp, index: u8, color: u16) {
    // CRAM Write command: 000011 (3)
    // CD1-0 = 11 -> 0xC000
    // CD5-2 = 0000 -> 0x0000
    // Address is index * 2
    let addr = (index as u16) * 2;
    vdp.write_control(0xC000 | (addr & 0x3FFF));
    vdp.write_control((addr >> 14) & 0x03);
    vdp.write_data(color);
}

// Helper to set register
fn set_register(vdp: &mut Vdp, reg: u8, val: u8) {
    vdp.write_control(0x8000 | ((reg as u16) << 8) | (val as u16));
}

#[test]
fn test_background_color() {
    let mut vdp = Vdp::new();

    // 1. Enable Display
    set_register(&mut vdp, 1, 0x40);

    // 2. Set Background Color to Palette 0, Color 0
    set_register(&mut vdp, 7, 0x00);

    // 3. Set Palette 0, Color 0 to White (0x0EEE)
    // 0x0EEE = 0000 1110 1110 1110
    // Blue=7, Green=7, Red=7
    write_cram(&mut vdp, 0, 0x0EEE);

    // 4. Render line 0
    vdp.render_line(0);

    // 5. Verify framebuffer (first pixel)
    // Expected White (0xFFFF) in RGB565
    assert_eq!(vdp.framebuffer[0], 0xFFFF, "Background should be white");

    // Test Black
    write_cram(&mut vdp, 0, 0x0000);
    vdp.render_line(0);
    assert_eq!(vdp.framebuffer[0], 0x0000, "Background should be black");
}

#[test]
fn test_background_color_selection() {
    let mut vdp = Vdp::new();

    // Enable Display
    set_register(&mut vdp, 1, 0x40);

    // Set Background Color to Palette 1, Color 2
    // Palette 1 = Index 16-31. Color 2 is index 18.
    // Reg 7 = (1 << 4) | 2 = 0x12.
    set_register(&mut vdp, 7, 0x12);

    // Set Palette 1, Color 2 to Red (0x00E)
    write_cram(&mut vdp, 18, 0x00E);

    // Set Palette 0, Color 0 to Blue (0xE00) to ensure we don't pick default
    write_cram(&mut vdp, 0, 0xE00);

    vdp.render_line(0);

    // Verify framebuffer
    // Red in RGB565: 0x00E -> R=7.
    // R=7 -> 31 (11111).
    // Shifted: 31 << 11 = 0xF800.
    assert_eq!(vdp.framebuffer[0], 0xF800, "Background should be Red");
}

#[test]
fn test_plane_rendering() {
    let mut vdp = Vdp::new();
    set_register(&mut vdp, 1, 0x40); // Enable Display
    set_register(&mut vdp, 2, 0x30); // Plane A Table at 0xC000 (0x30 << 10 = 0xC000)

    // Define a pattern (tile) 1.
    // 8x8 pixels. 32 bytes.
    // Top 4 lines: Color 1. Bottom 4 lines: Color 2.
    let mut pattern = vec![0u16; 16]; // 16 words = 32 bytes
    for i in 0..8 {
        pattern[i] = 0x1111; // Top 4 lines: Color 1
    }
    for i in 8..16 {
        pattern[i] = 0x2222; // Bottom 4 lines: Color 2
    }

    // Write pattern to VRAM at address 0x20 (Tile 1).
    write_vram(&mut vdp, 0x20, &pattern);

    // Write Nametable Entry at 0xC000 (0,0).
    // Tile=1. Palette=0.
    write_vram(&mut vdp, 0xC000, &[0x0001]);

    // Set Palette 0 colors.
    // Color 1: Red (0x00E).
    // Color 2: Green (0x0E0).
    // Background (Color 0): Black (0x000).
    write_cram(&mut vdp, 1, 0x00E);
    write_cram(&mut vdp, 2, 0x0E0);
    write_cram(&mut vdp, 0, 0x000);

    // Render Line 0 (Top half of tile). Should be Color 1 (Red).
    vdp.render_line(0);

    // Verify first 8 pixels.
    for i in 0..8 {
        assert_eq!(vdp.framebuffer[i], 0xF800, "Pixel {} should be Red", i);
    }

    // Render Line 4 (Bottom half of tile). Should be Color 2 (Green).
    vdp.render_line(4);
    let offset = 4 * 320;
    for i in 0..8 {
        assert_eq!(vdp.framebuffer[offset + i], 0x07E0, "Pixel {} should be Green", i);
    }

    // Verify next tile (pixels 8-15) is background (Black)
    // Note: This checks Line 4's next tile.
    for i in 8..16 {
        assert_eq!(vdp.framebuffer[offset + i], 0x0000, "Pixel {} should be Background", i);
    }
}

#[test]
fn test_sprite_rendering() {
    let mut vdp = Vdp::new();
    set_register(&mut vdp, 1, 0x40); // Enable Display

    // Set SAT address to 0xB800 (Reg 5 = 0x5C). `0x5C << 9` = `0xB800`.
    set_register(&mut vdp, 5, 0x5C);

    // Set Palette 0, Color 1 -> Red (0x00E)
    // Background (0,0) -> Black (0x000)
    write_cram(&mut vdp, 1, 0x00E);
    write_cram(&mut vdp, 0, 0x000);

    // Define Sprite 0.
    // V pos: 128 (screen y=0).
    // Size: 0x00 (1x1 tile).
    // Link: 0.
    // Priority: 1 (High). Palette: 0. Flip: 0. Base Tile: 1.
    // H pos: 128 (screen x=0).

    let sat_addr = 0xB800;
    // Word 0: V pos (bits 0-9). 128 = 0x080.
    write_vram(&mut vdp, sat_addr, &[0x0080]);

    // Word 1: Size (bits 8-11 in word? No, byte 2).
    // Byte 2 (Size): 0. Byte 3 (Link): 0.
    // Word at +2 is (Size << 8) | Link = 0x0000.
    write_vram(&mut vdp, sat_addr + 2, &[0x0000]);

    // Word 2: Attributes.
    // Priority=1 (bit 15). Palette=0. Flip=0. Base=1.
    // 0x8001.
    write_vram(&mut vdp, sat_addr + 4, &[0x8001]);

    // Word 3: H pos (bits 0-9). 128 = 0x080.
    write_vram(&mut vdp, sat_addr + 6, &[0x0080]);

    // Define Pattern 1.
    // Solid color 1.
    let mut pattern = vec![0u16; 16];
    for i in 0..16 {
        pattern[i] = 0x1111;
    }
    write_vram(&mut vdp, 0x20, &pattern);

    // Render Line 0.
    vdp.render_line(0);

    // Verify first 8 pixels are Red (Color 1).
    for i in 0..8 {
        assert_eq!(vdp.framebuffer[i], 0xF800, "Pixel {} should be Red (Sprite)", i);
    }

    // Verify pixel 8 is background (Black).
    assert_eq!(vdp.framebuffer[8], 0x0000, "Pixel 8 should be Background");
}

#[test]
fn test_priority() {
    let mut vdp = Vdp::new();
    set_register(&mut vdp, 1, 0x40); // Enable Display
    set_register(&mut vdp, 2, 0x30); // Plane A Table at 0xC000
    set_register(&mut vdp, 5, 0x5C); // SAT at 0xB800

    // Palette Setup
    // Color 1: Red (0x00E).
    // Color 2: Green (0x0E0).
    // Color 3: Blue (0xE00).
    write_cram(&mut vdp, 1, 0x00E);
    write_cram(&mut vdp, 2, 0x0E0);
    write_cram(&mut vdp, 3, 0xE00);

    // --- Plane A Setup ---
    // Pattern 1: Solid Color 1 (Red).
    let mut pattern_plane = vec![0u16; 16];
    for i in 0..16 { pattern_plane[i] = 0x1111; }
    write_vram(&mut vdp, 0x20, &pattern_plane);

    // Nametable Entry at 0xC000 (0,0).
    // Tile=1. Priority=1 (High).
    // 0x8001.
    write_vram(&mut vdp, 0xC000, &[0x8001]);

    // Nametable Entry at 0xC002 (1,0) (Pixel 8).
    // Tile=1. Priority=1 (High).
    write_vram(&mut vdp, 0xC002, &[0x8001]);

    // --- Sprite Setup ---
    let sat_addr = 0xB800;

    // Sprite 0 (Low Priority).
    // Pos (0,0). Size 1x1. Link 1. Priority 0. Palette 0. Tile 2.
    // V=128 (0x80).
    write_vram(&mut vdp, sat_addr, &[0x0080]);
    // Size=0, Link=1.
    write_vram(&mut vdp, sat_addr + 2, &[0x0001]);
    // Attr: Priority=0, Pal=0, Flip=0, Tile=2. -> 0x0002.
    write_vram(&mut vdp, sat_addr + 4, &[0x0002]);
    // H=128 (0x80).
    write_vram(&mut vdp, sat_addr + 6, &[0x0080]);

    // Sprite 1 (High Priority).
    // Pos (8,0). Size 1x1. Link 0. Priority 1. Palette 0. Tile 3.
    // V=128.
    write_vram(&mut vdp, sat_addr + 8, &[0x0080]);
    // Size=0, Link=0.
    write_vram(&mut vdp, sat_addr + 10, &[0x0000]);
    // Attr: Priority=1, Pal=0, Flip=0, Tile=3. -> 0x8003.
    write_vram(&mut vdp, sat_addr + 12, &[0x8003]);
    // H=136 (128+8) = 0x88.
    write_vram(&mut vdp, sat_addr + 14, &[0x0088]);

    // Pattern 2 (Sprite 0): Solid Color 2 (Green).
    let mut pattern_s0 = vec![0u16; 16];
    for i in 0..16 { pattern_s0[i] = 0x2222; }
    write_vram(&mut vdp, 0x40, &pattern_s0); // Tile 2 = 0x40.

    // Pattern 3 (Sprite 1): Solid Color 3 (Blue).
    let mut pattern_s1 = vec![0u16; 16];
    for i in 0..16 { pattern_s1[i] = 0x3333; }
    write_vram(&mut vdp, 0x60, &pattern_s1); // Tile 3 = 0x60.

    // Render
    vdp.render_line(0);

    // Check Pixel 0.
    // Plane A High (Red) vs Sprite Low (Green).
    // Plane A High is drawn AFTER Sprite Low.
    // Expect Red.
    assert_eq!(vdp.framebuffer[0], 0xF800, "Pixel 0 should be Red (Plane A High > Sprite Low)");

    // Check Pixel 8.
    // Plane A High (Red) vs Sprite High (Blue).
    // Sprite High is drawn AFTER Plane A High.
    // Expect Blue.
    assert_eq!(vdp.framebuffer[8], 0x001F, "Pixel 8 should be Blue (Sprite High > Plane A High)");
}
