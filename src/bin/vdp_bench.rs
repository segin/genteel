use genteel::vdp::Vdp;
use std::time::Instant;

fn main() {
    let mut vdp = Vdp::new();

    // Enable display
    vdp.registers[1] |= 0x40;

    // Set Plane A to 0xC000 (Reg 2 = 0x30)
    vdp.registers[2] = 0x30;
    // Set Plane B to 0xE000 (Reg 4 = 0x07)
    vdp.registers[4] = 0x07;
    // Set Sprite Table to 0xF800 (Reg 5 = 0x7C) - (0x7C << 9) & 0xFE00 = 0xF800
    vdp.registers[5] = 0x7C;

    // Set Plane Size to 64x64 (Reg 16 = 0x11)
    vdp.registers[16] = 0x11;

    // Fill VRAM with pattern data
    for i in 0..0x10000 {
        vdp.vram[i] = (i as u8).wrapping_mul(17);
    }

    // Initialize CRAM to avoid black screen logic (though render_line doesn't optimize black)
    for i in 0..64 {
        vdp.cram_cache[i] = 0xFFFF;
    }

    println!("Starting VDP render benchmark...");
    let start = Instant::now();
    let iterations = 1000;
    let height = 224;

    for _ in 0..iterations {
        for line in 0..height {
            vdp.render_line(line);
        }
    }

    let duration = start.elapsed();
    println!("Time for {} frames: {:?}", iterations, duration);
    let fps = (iterations as f64) / duration.as_secs_f64();
    println!("FPS: {:.2}", fps);
    println!("Time per frame: {:.2} ms", duration.as_millis() as f64 / iterations as f64);
}
