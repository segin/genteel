use genteel::apu::ym2612::{Bank, Ym2612};
use std::time::Instant;

fn main() {
    let mut ym = Ym2612::new();

    // Setup 6 channels with different frequencies
    // Channel 1: Bank 0
    ym.write_addr(Bank::Bank0, 0xA4); // Block 4, F-Num High 2
    ym.write_data_bank(Bank::Bank0, 0x22);
    ym.write_addr(Bank::Bank0, 0xA0); // F-Num Low
    ym.write_data_bank(Bank::Bank0, 0x55);

    // Channel 2: Bank 0
    ym.write_addr(Bank::Bank0, 0xA5);
    ym.write_data_bank(Bank::Bank0, 0x23);
    ym.write_addr(Bank::Bank0, 0xA1);
    ym.write_data_bank(Bank::Bank0, 0x66);

    // Channel 3: Bank 0
    ym.write_addr(Bank::Bank0, 0xA6);
    ym.write_data_bank(Bank::Bank0, 0x24);
    ym.write_addr(Bank::Bank0, 0xA2);
    ym.write_data_bank(Bank::Bank0, 0x77);

    // Channel 4: Bank 1
    ym.write_addr(Bank::Bank1, 0xA4);
    ym.write_data_bank(Bank::Bank1, 0x25);
    ym.write_addr(Bank::Bank1, 0xA0);
    ym.write_data_bank(Bank::Bank1, 0x88);

    // Channel 5: Bank 1
    ym.write_addr(Bank::Bank1, 0xA5);
    ym.write_data_bank(Bank::Bank1, 0x26);
    ym.write_addr(Bank::Bank1, 0xA1);
    ym.write_data_bank(Bank::Bank1, 0x99);

    // Channel 6: Bank 1
    ym.write_addr(Bank::Bank1, 0xA6);
    ym.write_data_bank(Bank::Bank1, 0x27);
    ym.write_addr(Bank::Bank1, 0xA2);
    ym.write_data_bank(Bank::Bank1, 0xAA);

    // Warmup
    for _ in 0..1000 {
        ym.generate_sample();
    }

    let start = Instant::now();
    let iterations = 10_000_000;

    for _ in 0..iterations {
        ym.generate_sample(); // 10 million iterations
    }

    let duration = start.elapsed();
    println!("Time for {} iterations: {:?}", iterations, duration);
}
