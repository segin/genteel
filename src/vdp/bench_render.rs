use super::*;
use std::time::Instant;

#[test]
fn bench_render_line_performance() {
    let mut vdp = Vdp::new();
    // Enable Display: Reg 1, bit 6 (0x40)
    vdp.registers[1] = 0x40;

    // Set up some sprites
    // Sprite Table at 0xD000 (Reg 5 = 0x68)
    vdp.registers[5] = 0x68;
    let sat_base = 0xD000;

    // Create 20 sprites on the same line to stress test
    for i in 0..20 {
        let addr = sat_base + (i * 8);
        vdp.vram[addr] = 0x00; vdp.vram[addr+1] = 128 + 10; // Y = 10
        vdp.vram[addr+2] = 0x05; // Size 2x2 tiles (bits 2,3 = 01 -> 2 tiles W; bits 0,1 = 01 -> 2 tiles H)
        vdp.vram[addr+3] = (i + 1) as u8; // Link to next
        vdp.vram[addr+4] = 0x00; vdp.vram[addr+5] = 0x00; // Attr
        vdp.vram[addr+6] = 0x00; vdp.vram[addr+7] = 128 + (i as u8 * 10); // X position
    }
    // Last sprite link 0
    let last_addr = sat_base + (19 * 8);
    vdp.vram[last_addr+3] = 0;

    // Fill VRAM with patterns to ensure fetching happens
    for i in 0..0x1000 {
        vdp.vram[i] = (i % 256) as u8;
    }

    let start = Instant::now();
    let iterations = 10000;

    for _ in 0..iterations {
        // Render line 10 where sprites are
        vdp.render_line(10);
    }

    let duration = start.elapsed();
    println!("Render Line ({} iterations) took: {:?}", iterations, duration);
}
