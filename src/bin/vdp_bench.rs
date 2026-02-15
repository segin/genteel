use genteel::vdp::Vdp;
use std::time::Instant;

fn main() {
    let mut vdp = Vdp::new();

    // Enable display
    vdp.registers[1] |= 0x40;

    // Set Plane A to 0xC000 (Reg 2 = 0x30)
    vdp.registers[2] = 0x30;

    // Palette 0, Color 1: Red
    vdp.write_control(0xC000); // Access CRAM addr 0
    vdp.write_control(0x0000);
    vdp.write_data(0xF800);

    // Set Tile 1 to solid Color 1
    for i in 0..16 {
        vdp.vram[32 + i] = 0x11;
    }

    // Set Nametable Entry (0,0) to Tile 1
    vdp.vram[0xC000] = 0x00;
    vdp.vram[0xC001] = 0x01;

    let iterations = 1000;
    println!("Benchmarking VDP rendering for {} frames...", iterations);

    let start = Instant::now();

    for _ in 0..iterations {
        for line in 0..224 {
            vdp.render_line(line);
        }
    }

    let duration = start.elapsed();
    println!("Time for {} frames: {:?}", iterations, duration);
    let fps = (iterations as f64) / duration.as_secs_f64();
    println!("FPS: {:.2}", fps);
    println!(
        "Time per frame: {:.2} ms",
        duration.as_millis() as f64 / iterations as f64
    );
}
