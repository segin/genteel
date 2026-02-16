use genteel::vdp::Vdp;
use std::time::Instant;

fn main() {
    let mut vdp = Vdp::new();

    // Setup sprite table at 0x0000
    // REG_SPRITE_TABLE is register 5. Value is address / 0x200.
    // So 0 -> 0x0000.
    vdp.registers[5] = 0;

    // Setup 80 sprites linked together
    for i in 0..80 {
        let addr = i * 8;
        let next_link = if i == 79 { 0 } else { i + 1 };

        // Y position (0..=0x3FF) - centered around 128
        // Let's scatter them vertically so some are visible on some lines
        let y_pos = 128 + (i as u16 * 2);
        vdp.vram[addr] = (y_pos >> 8) as u8;
        vdp.vram[addr + 1] = (y_pos & 0xFF) as u8;

        // Size (Bits 0-1: V-size-1, Bits 2-3: H-size-1)
        // Let's make them 2x2 tiles (size=5: 0101)
        vdp.vram[addr + 2] = 0x05;

        // Link
        vdp.vram[addr + 3] = next_link as u8;

        // Priority/Palette/Flip/BaseTile
        // High priority on even sprites, low on odd
        let priority = if i % 2 == 0 { 0x80 } else { 0x00 };
        vdp.vram[addr + 4] = priority;
        vdp.vram[addr + 5] = 0x00; // Tile 0

        // X position
        let x_pos = 128 + (i as u16 * 3);
        vdp.vram[addr + 6] = (x_pos >> 8) as u8;
        vdp.vram[addr + 7] = (x_pos & 0xFF) as u8;
    }

    // Enable display
    vdp.registers[1] |= 0x40; // MODE2_DISPLAY_ENABLE

    // Warmup
    for _ in 0..100 {
        vdp.render_line(100);
    }

    let start = Instant::now();
    let iterations = 10_000;

    for _ in 0..iterations {
        // Render a line where many sprites are visible
        vdp.render_line(150);
    }

    let duration = start.elapsed();
    println!("Time for {} iterations: {:?}", iterations, duration);
}
