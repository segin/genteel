use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Halted,
    Breakpoint,
    Step,
    Interrupt,
}

impl StopReason {
    pub fn signal(&self) -> u8 {
        match self {
            StopReason::Halted => 5,
            StopReason::Breakpoint => 5,
            StopReason::Step => 5,
            StopReason::Interrupt => 2,
        }
    }

    pub fn signal_string(&self) -> &'static str {
        match self {
            StopReason::Halted => "S05",
            StopReason::Breakpoint => "S05",
            StopReason::Step => "S05",
            StopReason::Interrupt => "S02",
        }
    }
}

fn main() {
    // Basic verification tests
    assert_eq!(StopReason::Breakpoint.signal_string(), "S05");
    assert_eq!(StopReason::Step.signal_string(), "S05");
    assert_eq!(StopReason::Interrupt.signal_string(), "S02");

    let iterations = 10_000_000;
    let reason = StopReason::Breakpoint;

    // Benchmark dynamic formatting
    let start_dynamic = Instant::now();
    for _ in 0..iterations {
        let formatted = format!("S{:02x}", reason.signal());
        std::hint::black_box(formatted);
    }
    let duration_dynamic = start_dynamic.elapsed();

    // Benchmark static string return
    let start_static = Instant::now();
    for _ in 0..iterations {
        let static_str = reason.signal_string();
        std::hint::black_box(static_str);
    }
    let duration_static = start_static.elapsed();

    // Benchmark static to_string
    let start_to_string = Instant::now();
    for _ in 0..iterations {
        let allocated_str = reason.signal_string().to_string();
        std::hint::black_box(allocated_str);
    }
    let duration_to_string = start_to_string.elapsed();

    println!("Iterations: {}", iterations);
    println!("Dynamic `format!`: {:?}", duration_dynamic);
    println!("Static `&'static str`: {:?}", duration_static);
    println!("Static `.to_string()`: {:?}", duration_to_string);
    println!();

    let dyn_secs = duration_dynamic.as_secs_f64();
    let stat_secs = duration_static.as_secs_f64();
    let to_str_secs = duration_to_string.as_secs_f64();

    if stat_secs > 0.0 {
        println!(
            "Improvement (Static over Dynamic): {:.2}x",
            dyn_secs / stat_secs
        );
    }
    if to_str_secs > 0.0 {
        println!(
            "Improvement (.to_string() over Dynamic): {:.2}x",
            dyn_secs / to_str_secs
        );
    }
}
