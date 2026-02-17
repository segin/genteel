use genteel::apu::ym2612::{Bank, Ym2612};
use std::time::Instant;

fn main() {
    let mut ym = Ym2612::new();

    // Set frequencies for all 6 channels to ensure they are active
    // Bank 0: Ch 1, 2, 3
    for ch in 0..3 {
        // Set F-Num Low to 0x55
        ym.write_addr(Bank::Bank0, 0xA0 + ch as u8);
        ym.write_data_bank(Bank::Bank0, 0x55);
        // Set F-Num High + Block to 0x22 (Block 4, Hi 2)
        ym.write_addr(Bank::Bank0, 0xA4 + ch as u8);
        ym.write_data_bank(Bank::Bank0, 0x22);

        // Set Volume to max (TL=0)
        ym.write_addr(Bank::Bank0, 0x40 + ch as u8); // DT1/MUL
        ym.write_data_bank(Bank::Bank0, 0x01); // Multiply by 1

        ym.write_addr(Bank::Bank0, 0x4C + ch as u8); // Total Level
        ym.write_data_bank(Bank::Bank0, 0x00); // 0 attenuation
    }

    // Bank 1: Ch 4, 5, 6
    for ch in 0..3 {
        // Set F-Num Low to 0x55
        ym.write_addr(Bank::Bank1, 0xA0 + ch as u8);
        ym.write_data_bank(Bank::Bank1, 0x55);
        // Set F-Num High + Block to 0x22
        ym.write_addr(Bank::Bank1, 0xA4 + ch as u8);
        ym.write_data_bank(Bank::Bank1, 0x22);

        // Set Volume
        ym.write_addr(Bank::Bank1, 0x40 + ch as u8);
        ym.write_data_bank(Bank::Bank1, 0x01);

        ym.write_addr(Bank::Bank1, 0x4C + ch as u8);
        ym.write_data_bank(Bank::Bank1, 0x00);
    }

    let samples_to_generate = 5_000_000;
    let start = Instant::now();

    for _ in 0..samples_to_generate {
        ym.generate_sample();
    }

    let duration = start.elapsed();
    println!(
        "Benchmark: {} samples took {:?}",
        samples_to_generate, duration
    );
    println!(
        "Samples per second: {:.2} M samples/sec",
        (samples_to_generate as f64 / duration.as_secs_f64()) / 1_000_000.0
    );
}
