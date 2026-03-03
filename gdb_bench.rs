use std::fmt::Write;
use std::time::Instant;

fn main() {
    let iters = 10_000_000;

    // Test 1: format!
    let start = Instant::now();
    let mut sum1 = 0;
    for _ in 0..iters {
        let s = format!("S{:02x}", 5);
        sum1 += s.len();
    }
    let d1 = start.elapsed();
    println!("format!: {:?} (sum={})", d1, sum1);

    // Test 2: pre-allocated String + Write
    let start = Instant::now();
    let mut sum2 = 0;
    let mut buf = String::with_capacity(4);
    for _ in 0..iters {
        buf.clear();
        let _ = write!(&mut buf, "S{:02x}", 5);
        sum2 += buf.len();
    }
    let d2 = start.elapsed();
    println!("pre-allocated: {:?} (sum={})", d2, sum2);
}
