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

    let start = Instant::now();
    let iterations = 10000;

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
