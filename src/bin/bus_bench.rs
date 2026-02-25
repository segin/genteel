use genteel::memory::bus::Bus;
use std::time::Instant;

fn main() {
    let mut bus = Bus::new();
    // Load a 1MB ROM
    let rom = vec![0xAA; 1024 * 1024];
    bus.load_rom(&rom);

    let start = Instant::now();
    let iterations = 100_000_000;

    // We want to verify the result to prevent optimization,
    // but without printing it every time.
    let mut accumulator: u64 = 0;

    for i in 0..iterations {
        // Access addresses within ROM range (0x000000 - 0x3FFFFF)
        // using a bit of randomness or pattern to avoid branch prediction over-optimization if possible,
        // but simple sequential or modulo access is usually fine for this specific test.
        let addr = (i as u32) % 0x100000;
        let val = bus.read_byte(addr);
        accumulator = accumulator.wrapping_add(val as u64);
    }

    let duration = start.elapsed();
    println!("Bus Read Byte Benchmark");
    println!("Iterations: {}", iterations);
    println!("Time: {:?}", duration);
    println!(
        "M Ops/sec: {:.2}",
        (iterations as f64 / duration.as_secs_f64()) / 1_000_000.0
    );
    println!("Accumulator: {}", accumulator);
}
