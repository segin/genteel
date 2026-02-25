use super::decoder::decode;
use std::time::Instant;

#[test]
fn bench_decode_performance() {
    let mut opcodes = Vec::with_capacity(65536);
    for i in 0..=65535 {
        opcodes.push(i as u16);
    }

    // Warmup
    for &opcode in &opcodes {
        let _ = decode(opcode);
    }

    let iterations = 1000;
    let start = Instant::now();

    for _ in 0..iterations {
        for &opcode in &opcodes {
            let _ = decode(opcode);
        }
    }

    let duration = start.elapsed();
    println!(
        "Decode 64K opcodes ({} iterations) took: {:?}",
        iterations, duration
    );
    let total_decodes = iterations as u128 * 65536;
    let nanos_per_decode = duration.as_nanos() / total_decodes;
    println!("Average time per decode: {} ns", nanos_per_decode);
}
