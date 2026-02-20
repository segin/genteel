#[test]
fn test_render_tile_zero() {
    let mut vdp = super::Vdp::new();
    
    // Enable Display: Reg 1, bit 6 (0x40)
    vdp.registers[1] = 0x40;
    // Plane A Address: 0xC000 (Reg 2 = 0x30)
    vdp.registers[2] = 0x30;
    // Plane Size: 32x32 (Reg 16 = 0x00)
    vdp.registers[16] = 0x00;
    
    // Background color: Palette 0, Color 0 (Black initially)
    // We'll set Palette 0, Color 1 to Red (0xF800)
    vdp.cram_cache[1] = 0xF800;
    
    // Tile 0 Pattern: All pixels Color 1 (Red)
    // On Genesis, Tile 0 is at VRAM 0x0000
    for i in 0..32 {
        vdp.vram[i] = 0x11; // 2 pixels of color 1 per byte
    }
    
    // Nametable Entry at 0xC000 (0,0) -> Tile 0, Pal 0, Priority 0
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x00;
    
    // Render Line 0
    vdp.render_line(0);
    
    // Check if first pixel is Red (from Tile 0)
    // If Tile 0 is skipped, it will be 0x0000 (Black background)
    assert_eq!(vdp.framebuffer[0], 0xF800, "Tile 0 was not rendered!");
}

#[test]
fn test_render_plane_b_tile_zero() {
    let mut vdp = super::Vdp::new();
    
    // Enable Display: Reg 1, bit 6 (0x40)
    vdp.registers[1] = 0x40;
    // Plane A Address: 0xC000 (to avoid overlapping with Tile 0 and H-scroll)
    vdp.registers[2] = 0x30;
    // Plane B Address: 0xE000 (Reg 4 = 0x07)
    vdp.registers[4] = 0x07;
    // Plane Size: 32x32 (Reg 16 = 0x00)
    vdp.registers[16] = 0x00;
    // H-Scroll Address: 0xD000 (Reg 13 = 0x34)
    vdp.registers[13] = 0x34;
    
    // Background color: Palette 0, Color 0 (Black initially)
    // We'll set Palette 1, Color 1 to Green (0x07E0)
    vdp.cram_cache[17] = 0x07E0;
    
    // Tile 0 Pattern: All pixels Color 1
    for i in 0..32 {
        vdp.vram[i] = 0x11;
    }
    
    // Ensure H-scroll is 0 for both planes at 0xD000
    // (VDP reads 4 bytes for HS: Plane A HS, then Plane B HS)
    vdp.vram[0xD000] = 0;
    vdp.vram[0xD001] = 0;
    vdp.vram[0xD002] = 0;
    vdp.vram[0xD003] = 0;
    
    // Nametable Entry at 0xE000 (0,0) -> Tile 0, Pal 1, Priority 0
    vdp.vram[0xE000] = 0x20; // Palette 1 (bits 13-14)
    vdp.vram[0xE001] = 0x00;
    
    // Ensure Plane A nametable at 0xC000 points to a different (empty) tile
    // Tile 1 is at VRAM 0x0020. It's already 0 if we don't touch it.
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01; // Tile 1
    
    // Render Line 0
    vdp.render_line(0);
    
    // Check if first pixel is Green (from Tile 0 in Plane B)
    assert_eq!(vdp.framebuffer[0], 0x07E0, "Plane B Tile 0 was not rendered!");
}
