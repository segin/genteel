#[cfg(test)]
mod performance_tests {
    use crate::memory::bus::Bus;
    use std::time::Instant;

    #[test]
    fn benchmark_dma_transfer() {
        let mut bus = Bus::new();
        // Setup 64KB ROM
        let rom_size = 0x10000;
        let mut rom = vec![0; rom_size];
        for i in 0..rom_size {
            rom[i] = (i % 256) as u8;
        }
        bus.load_rom(&rom);

        // Setup DMA
        // Length: 0x7FFF words (64KB - 2 bytes)
        let len_words = 0x7FFF;
        bus.vdp.registers[19] = (len_words & 0xFF) as u8;
        bus.vdp.registers[20] = (len_words >> 8) as u8;

        // Source: 0 (ROM)
        bus.vdp.registers[21] = 0;
        bus.vdp.registers[22] = 0;
        bus.vdp.registers[23] = 0; // Mode 0 (Transfer)

        // Set Auto-Increment to 2 (standard VRAM write)
        bus.vdp.registers[15] = 2;

        // Enable DMA
        bus.vdp.registers[1] |= 0x10;

        // Trigger via Control Port (VRAM Write 0x0000)
        bus.write_word(0xC00004, 0x4000);
        bus.write_word(0xC00004, 0x0080); // DMA trigger

        // The DMA actually runs inside write_word -> run_dma

        let iterations = 1000;
        let start = Instant::now();

        for _ in 0..iterations {
            bus.vdp.dma_pending = true;
            bus.write_word(0xC00004, 0x4000); // First word
            bus.write_word(0xC00004, 0x0080); // Second word, triggers DMA
        }

        let duration = start.elapsed();
        println!(
            "DMA Benchmark: {:?} for {} iterations of 64KB transfer",
            duration, iterations
        );
    }
}
