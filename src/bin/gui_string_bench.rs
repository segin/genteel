use std::fmt::Write;
use std::time::Instant;

fn main() {
    let iterations = 10_000_000;

    // Test 1: format!
    let start = Instant::now();
    let mut total_len = 0;
    for _ in 0..iterations {
        for slot in 0..10 {
            let s = format!("Slot {}", slot);
            total_len += s.len();
        }
    }
    let duration1 = start.elapsed();
    println!("format! time: {:?}", duration1);

    // Test 2: pre-allocated string buffer
    let start = Instant::now();
    let mut buf = String::with_capacity(16);
    for _ in 0..iterations {
        for slot in 0..10 {
            buf.clear();
            let _ = write!(&mut buf, "Slot {}", slot);
            total_len += buf.len();
        }
    }
    let duration2 = start.elapsed();
    println!("pre-allocated buf time: {:?}", duration2);

    // Test 3: static array
    const SLOT_LABELS: [&str; 10] = [
        "Slot 0", "Slot 1", "Slot 2", "Slot 3", "Slot 4", "Slot 5", "Slot 6", "Slot 7", "Slot 8",
        "Slot 9",
    ];
    let start = Instant::now();
    for _ in 0..iterations {
        for slot in 0..10 {
            let s = SLOT_LABELS[slot as usize];
            total_len += s.len();
        }
    }
    let duration3 = start.elapsed();
    println!("static array time: {:?}", duration3);

    println!("total_len: {}", total_len);
}
