use super::*;
use std::time::Instant;

#[test]
fn bench_dma_fill_performance() {
    let mut vdp = Vdp::new();
    // Setup DMA fill
    vdp.write_control(0x8114); // Enable DMA
    vdp.write_control(0x9780); // DMA Fill
    vdp.write_control(0x8F01); // Auto-inc 1
                               // Set length to max (0xFFFF)
    vdp.write_control(0x93FF);
    vdp.write_control(0x94FF);

    // Set destination 0
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    // Set data
    vdp.write_data(0xAA00);

    // Run the fill repeatedly
    let start = Instant::now();
    let iterations = 10000;

    // execute_dma returns length, but doesn't modify length registers.
    // It uses length registers to determine loop count.
    // We want to measure the execution time of the loop inside execute_dma.

    for _ in 0..iterations {
        vdp.execute_dma();
    }

    let duration = start.elapsed();
    println!("DMA Fill ({} iterations) took: {:?}", iterations, duration);
}

#[test]
fn bench_dma_fill_write_data() {
    let mut vdp = Vdp::new();
    // Enable DMA (bit 4 of Mode 2)
    vdp.registers[REG_MODE2] = MODE2_DMA_ENABLE;
    // Set DMA Mode to Fill (bit 7 of DMA SRC HI)
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    // Set Auto Increment to 1
    vdp.registers[REG_AUTO_INC] = 1;
    // Set DMA Length to 0xFFFF (max)
    vdp.registers[REG_DMA_LEN_HI] = 0xFF;
    vdp.registers[REG_DMA_LEN_LO] = 0xFF;

    let iterations = 1000;
    let start = Instant::now();

    for _ in 0..iterations {
        vdp.dma_pending = true;
        vdp.control_address = 0;
        // This triggers the loop in write_data
        vdp.write_data(0xAA00);
    }

    let duration = start.elapsed();
    println!(
        "DMA Fill via write_data ({} iterations) took: {:?}",
        iterations, duration
    );
}
