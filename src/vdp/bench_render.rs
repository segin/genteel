use super::*;
use std::time::Instant;

#[test]
fn bench_render_line_performance() {
    let mut vdp = Vdp::new();

    // Enable display (Reg 1 bit 6) and V30 (Reg 1 bit 3)
    vdp.registers[1] = 0x40 | 0x04;

    // Set plane sizes to 64x64 (Reg 16 = 0x11)
    vdp.registers[16] = 0x11;

    // Set Plane A base address (Reg 2 = 0x30 -> 0xC000)
    vdp.registers[2] = 0x30;

    // Set Plane B base address (Reg 4 = 0x07 -> 0xE000)
    vdp.registers[4] = 0x07;

    // Set Sprite Table at 0xD400 (Reg 5 = 0x6A)
    vdp.registers[5] = 0x6A;

    // Fill VRAM with random data to simulate patterns and nametables
    // We want a mix of tiles to ensure we aren't just hitting zeros
    for i in 0..0x10000 {
        vdp.vram[i] = (i & 0xFF) as u8;
    }

    // Fill CRAM
    for i in 0..128 {
        vdp.cram[i] = (i & 0xFF) as u8;
    }
    // Update cram cache with some colors so we actually write to framebuffer
    for i in 0..64 {
        vdp.cram_cache[i] = (i as u16) * 0x0400 + 0x0020;
    }

    // Setup Sprites
    let sat_base = 0xD400;

    // Create 40 linked sprites
    for i in 0..40 {
        let addr = sat_base + (i * 8);
        // V Pos: center around line 100 (visible)
        // Line 100 + 128 = 228
        let y = 90 + (i % 20);
        vdp.vram[addr] = 0x00;
        vdp.vram[addr + 1] = (y + 128) as u8;

        // Size: 0 (1x1)
        vdp.vram[addr + 2] = 0x00;

        // Link: i + 1. Last one links to 0.
        let link = if i == 39 { 0 } else { i + 1 };
        vdp.vram[addr + 3] = link as u8;

        // Attr: Priority mixed. Palette 0.
        // Priority bit is 0x8000. Byte 4 bit 7.
        let priority = if i % 2 == 0 { 0x80 } else { 0x00 };
        vdp.vram[addr + 4] = priority; // Palette 0, Priority
        vdp.vram[addr + 5] = 0x00; // Tile 0

        // H Pos: Spread across 320 pixels.
        // 128 + (i * 8)
        let x = 128 + (i * 8);
        vdp.vram[addr + 6] = (x >> 8) as u8;
        vdp.vram[addr + 7] = (x & 0xFF) as u8;
    }

    let start = Instant::now();
    let iterations = 100000;

    for _ in 0..iterations {
        // Render line 100
        vdp.render_line(100);
    }

    let duration = start.elapsed();
    println!(
        "Render Line ({} iterations) took: {:?}",
        iterations, duration
    );
}

#[test]
fn bench_render_line_sparse() {
    let mut vdp = Vdp::new();
    vdp.registers[1] = 0x40 | 0x04;
    vdp.registers[16] = 0x11;
    vdp.registers[2] = 0x30;
    vdp.registers[4] = 0x07;
    vdp.registers[5] = 0x6A;

    // Fill VRAM with 50% zeros
    for i in 0..0x10000 {
        if i % 2 == 0 {
            vdp.vram[i] = (i & 0xFF) as u8;
        } else {
            vdp.vram[i] = 0;
        }
    }
    // Fill CRAM
    for i in 0..128 {
        vdp.cram[i] = (i & 0xFF) as u8;
    }
    for i in 0..64 {
        vdp.cram_cache[i] = (i as u16) * 0x0400 + 0x0020;
    }

    let sat_base = 0xD400;
    for i in 0..40 {
        let addr = sat_base + (i * 8);
        let y = 90 + (i % 20);
        vdp.vram[addr] = 0x00;
        vdp.vram[addr + 1] = (y + 128) as u8;
        vdp.vram[addr + 2] = 0x00;
        let link = if i == 39 { 0 } else { i + 1 };
        vdp.vram[addr + 3] = link as u8;
        let priority = if i % 2 == 0 { 0x80 } else { 0x00 };
        vdp.vram[addr + 4] = priority;
        vdp.vram[addr + 5] = 0x00;
        let x = 128 + (i * 8);
        vdp.vram[addr + 6] = (x >> 8) as u8;
        vdp.vram[addr + 7] = (x & 0xFF) as u8;
    }

    let start = Instant::now();
    let iterations = 100000;

    for _ in 0..iterations {
        vdp.render_line(100);
    }

    let duration = start.elapsed();
    println!(
        "Render Line Sparse ({} iterations) took: {:?}",
        iterations, duration
    );
}
