use std::fmt::Write;
use std::time::Instant;

fn main() {
    let m68k_disasm = vec![
        (0x000200, "MOVE.W D0, D1".to_string()),
        (0x000202, "ADD.L D2, D3".to_string()),
        (0x000204, "SUB.B #$10, D4".to_string()),
        (0x000206, "NOP".to_string()),
        (0x000208, "RTE".to_string()),
        (0x00020A, "BRA.S $000210".to_string()),
        (0x00020C, "MOVEA.L A0, A1".to_string()),
        (0x00020E, "JMP (A2)".to_string()),
        (0x000210, "BSR.W $000300".to_string()),
        (0x000214, "RTS".to_string()),
    ];

    let m68k_pc = 0x000208;
    let iterations = 1_000_000;

    let start_baseline = Instant::now();
    for _ in 0..iterations {
        for (addr, text) in &m68k_disasm {
            let is_current = *addr == m68k_pc;
            let label = format!("{:06X}: {}", addr, text);
            if is_current {
                let _final_label = format!("-> {}", label);
                // fake usage
                std::hint::black_box(_final_label);
            } else {
                let _final_label = format!("   {}", label);
                // fake usage
                std::hint::black_box(_final_label);
            }
        }
    }
    let baseline_duration = start_baseline.elapsed();

    let start_optimized = Instant::now();
    let mut buffer = String::with_capacity(64);
    for _ in 0..iterations {
        for (addr, text) in &m68k_disasm {
            let is_current = *addr == m68k_pc;
            buffer.clear();
            if is_current {
                let _ = write!(&mut buffer, "-> {:06X}: {}", addr, text);
                std::hint::black_box(buffer.as_str());
            } else {
                let _ = write!(&mut buffer, "   {:06X}: {}", addr, text);
                std::hint::black_box(buffer.as_str());
            }
        }
    }
    let optimized_duration = start_optimized.elapsed();

    println!("Baseline duration: {:?}", baseline_duration);
    println!("Optimized duration: {:?}", optimized_duration);
    println!(
        "Improvement: {:.2}x",
        baseline_duration.as_secs_f64() / optimized_duration.as_secs_f64()
    );
}
