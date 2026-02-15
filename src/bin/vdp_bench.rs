use genteel::vdp::Vdp;
use std::time::Instant;

fn main() {
    let mut vdp = Vdp::new();

    // Setup some dummy data in VRAM
    // Nametables at 0xC000 (default for Plane A) and 0xE000 (Plane B)
    // 0xC000 / 2 = 0x6000
    // Fill VRAM with some patterns
    for i in 0..0x10000 {
        vdp.vram[i] = (i % 256) as u8;
    }

    // Enable display
    vdp.registers[1] |= 0x40;

    // Set Plane A to 0xC000 (default is usually different, let's set it explicitly)
    // Reg 2: 0x30 -> 0xC000
    vdp.registers[2] = 0x30;

    // Set Plane B to 0xE000
    // Reg 4: 0x07 -> 0xE000
    vdp.registers[4] = 0x07;

    // Set size to 64x64
    vdp.registers[16] = 0x11;

    let start = Instant::now();
    let iterations = 1000; // Reduced iterations to keep it fast

    for _ in 0..iterations {
        // Render a full frame (224 lines)
        for line in 0..224 {
            vdp.render_line(line);
        }
    }

    let duration = start.elapsed();
    println!("Time for {} frames: {:?}", iterations, duration);
    println!("FPS: {}", iterations as f64 / duration.as_secs_f64());
}
