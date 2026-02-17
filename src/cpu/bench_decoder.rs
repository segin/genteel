use super::decoder::{decode, Instruction};
use std::time::Instant;
use std::hint::black_box;

struct Lcg {
    state: u64,
}
impl Lcg {
    fn new(seed: u64) -> Self { Self { state: seed } }
    fn next_u16(&mut self) -> u16 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 48) as u16
    }
}

#[test]
fn bench_decode_performance() {
    println!("Size of Instruction: {} bytes", std::mem::size_of::<Instruction>());

    let mut opcodes = Vec::with_capacity(65536);
    for i in 0..=65535 {
        opcodes.push(i as u16);
    }

    // Warmup
    for &opcode in &opcodes {
        black_box(decode(black_box(opcode)));
    }

    let iterations = 1000;
    let start = Instant::now();

    for _ in 0..iterations {
        for &opcode in &opcodes {
            black_box(decode(black_box(opcode)));
        }
    }

    let duration = start.elapsed();
    println!("Sequential: Decode 64K opcodes ({} iterations) took: {:?}", iterations, duration);
    let total_decodes = iterations as u128 * 65536;
    let nanos_per_decode = duration.as_nanos() / total_decodes;
    println!("Average time per decode (sequential): {} ns", nanos_per_decode);

    // Random access benchmark
    let mut rng = Lcg::new(12345);
    let mut random_opcodes = Vec::with_capacity(1_000_000);
    for _ in 0..1_000_000 {
        random_opcodes.push(rng.next_u16());
    }

    let start_random = Instant::now();
    for &opcode in &random_opcodes {
        black_box(decode(black_box(opcode)));
    }
    let duration_random = start_random.elapsed();
    let total_random_decodes = 1_000_000u128;
    println!("Random: Decode 1M opcodes took: {:?}", duration_random);
    println!("Average time per decode (random): {} ns", duration_random.as_nanos() / total_random_decodes);
}
