use std::time::Instant;
use std::fmt::Write;

fn main() {
    let iters = 1_000_000;

    // Setup dummy data
    let d = [0x12345678u32; 8];
    let a = [0x87654321u32; 8];

    // Baseline format!
    let start = Instant::now();
    for _ in 0..iters {
        for i in 0..8 {
            let _s1 = format!("D{}: {:08X}", i, d[i]);
            let _s2 = format!("A{}: {:08X}", i, a[i]);
            std::hint::black_box((_s1, _s2));
        }
    }
    let baseline_time = start.elapsed();
    println!("Baseline format! time: {:?}", baseline_time);

    // Optimized with write!
    let start = Instant::now();
    let mut buf_d = String::with_capacity(16);
    let mut buf_a = String::with_capacity(16);
    for _ in 0..iters {
        for i in 0..8 {
            buf_d.clear();
            buf_a.clear();
            let _ = write!(&mut buf_d, "D{}: {:08X}", i, d[i]);
            let _ = write!(&mut buf_a, "A{}: {:08X}", i, a[i]);
            std::hint::black_box((buf_d.as_str(), buf_a.as_str()));
        }
    }
    let optimized_time = start.elapsed();
    println!("Optimized write! time: {:?}", optimized_time);

    println!("Improvement: {:.2}x", baseline_time.as_secs_f64() / optimized_time.as_secs_f64());
}
