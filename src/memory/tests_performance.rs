#[cfg(test)]
mod performance_tests {
    use crate::memory::bus::Bus;
    use crate::memory::z80_bus::Z80Bus;
    use crate::memory::{MemoryInterface, SharedBus};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Instant;

    #[test]
    fn benchmark_z80_bus_access() {
        let bus = Rc::new(RefCell::new(Bus::new()));
        let mut z80_bus = Z80Bus::new(SharedBus::new(bus.clone()));

        // Ensure Z80 bus request is active so we can access RAM
        bus.borrow_mut().z80_bus_request = true;

        let iterations = 10_000_000;
        let start = Instant::now();

        // Perform mix of reads and writes
        // Z80 RAM (0x0000-0x1FFF)
        // Banked Memory (0x8000-0xFFFF) - accessing 0x8000 maps to 68k address 0 (ROM)

        // Pre-load some data in ROM for banked access check
        bus.borrow_mut().load_rom(&vec![0xAA; 1024]);

        let mut sum: u32 = 0;

        for i in 0..iterations {
            // Write to Z80 RAM
            z80_bus.write_byte((i as u32) & 0x1FFF, (i & 0xFF) as u8);

            // Read from Z80 RAM
            sum = sum.wrapping_add(z80_bus.read_byte((i as u32) & 0x1FFF) as u32);

            // Read from Banked Memory (ROM)
            sum = sum.wrapping_add(z80_bus.read_byte(0x8000 + ((i as u32) & 0xFF)) as u32);
        }

        let duration = start.elapsed();
        println!(
            "Z80 Bus Benchmark (Safe): {:?} for {} iterations. Sum: {}",
            duration, iterations, sum
        );
    }

    #[test]
    fn benchmark_z80_bus_access_raw() {
        let bus = Rc::new(RefCell::new(Bus::new()));
        let mut z80_bus = Z80Bus::new(SharedBus::new(bus.clone()));

        // Ensure Z80 bus request is active so we can access RAM
        bus.borrow_mut().z80_bus_request = true;

        // Unsafe setup
        unsafe {
            z80_bus.set_raw_bus(bus.as_ptr());
        }

        let iterations = 10_000_000;
        let start = Instant::now();

        // Pre-load some data in ROM
        bus.borrow_mut().load_rom(&vec![0xAA; 1024]);

        let mut sum: u32 = 0;

        for i in 0..iterations {
            // Write to Z80 RAM
            z80_bus.write_byte((i as u32) & 0x1FFF, (i & 0xFF) as u8);

            // Read from Z80 RAM
            sum = sum.wrapping_add(z80_bus.read_byte((i as u32) & 0x1FFF) as u32);

            // Read from Banked Memory (ROM)
            sum = sum.wrapping_add(z80_bus.read_byte(0x8000 + ((i as u32) & 0xFF)) as u32);
        }

        let duration = start.elapsed();
        println!(
            "Z80 Bus Benchmark (Raw): {:?} for {} iterations. Sum: {}",
            duration, iterations, sum
        );

        // Clear raw pointer
        z80_bus.clear_raw_bus();
    }

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
        let len_words = 0x7FFF;
        bus.vdp.registers[19] = (len_words & 0xFF) as u8;
        bus.vdp.registers[20] = (len_words >> 8) as u8;
        bus.vdp.registers[21] = 0;
        bus.vdp.registers[22] = 0;
        bus.vdp.registers[23] = 0;
        bus.vdp.registers[15] = 2;
        bus.vdp.registers[1] |= 0x10;

        let iterations = 1000;
        let start = Instant::now();

        for _ in 0..iterations {
            bus.vdp.dma_pending = true;
            bus.write_word(0xC00004, 0x4000);
            bus.write_word(0xC00004, 0x0080);
        }

        let duration = start.elapsed();
        println!(
            "DMA Benchmark: {:?} for {} iterations of 64KB transfer",
            duration, iterations
        );
    }

    #[test]
    fn benchmark_vdp_long_access() {
        let mut bus = Bus::new();
        // Setup VDP for VRAM Write at address 0
        bus.write_word(0xC00004, 0x4000);
        bus.write_word(0xC00004, 0x0000);

        let iterations = 1_000_000;
        let start = Instant::now();

        for i in 0..iterations {
            bus.write_long(0xC00000, i);
        }

        let duration = start.elapsed();
        println!(
            "VDP Long Write Benchmark: {:?} for {} iterations",
            duration, iterations
        );

        // Setup VDP for VRAM Read at address 0
        bus.write_word(0xC00004, 0x0000);
        bus.write_word(0xC00004, 0x0000);

        let start = Instant::now();
        let mut sum: u32 = 0;
        for _ in 0..iterations {
            sum = sum.wrapping_add(bus.read_long(0xC00000));
        }

        let duration = start.elapsed();
        println!(
            "VDP Long Read Benchmark: {:?} for {} iterations. Sum: {}",
            duration, iterations, sum
        );
    }

    #[test]
    fn test_vdp_long_access_correctness() {
        let mut bus = Bus::new();

        // 1. Setup VDP Auto-Increment to 2
        bus.vdp.registers[15] = 2;

        // 2. Setup VRAM Write to address 0x0000
        // Command: 0x40000000 (VRAM Write)
        bus.write_word(0xC00004, 0x4000);
        bus.write_word(0xC00004, 0x0000);

        // 3. Write Long 0x11223344 to Data Port
        bus.write_long(0xC00000, 0x11223344);

        // Verify VRAM content
        // 0x0000: 0x11 (High byte of first word)
        // 0x0001: 0x22 (Low byte of first word)
        // 0x0002: 0x33 (High byte of second word)
        // 0x0003: 0x44 (Low byte of second word)
        assert_eq!(bus.vdp.vram[0], 0x11);
        assert_eq!(bus.vdp.vram[1], 0x22);
        assert_eq!(bus.vdp.vram[2], 0x33);
        assert_eq!(bus.vdp.vram[3], 0x44);

        // Verify address incremented by 4 (2 * 2 words)
        // Since VDP address register wraps, we can check it via control port read or internal state.
        // Let's use internal state via debug interface which is easier in test
        // But we are in bus test, so we can access bus.vdp directly.
        // Wait, Vdp struct fields are public in crate, but `control_address` is public? Yes.
        assert_eq!(bus.vdp.control_address, 4);

        // 4. Setup VRAM Read from address 0x0000
        bus.write_word(0xC00004, 0x0000);
        bus.write_word(0xC00004, 0x0000);

        // 5. Read Long from Data Port
        let val = bus.read_long(0xC00000);

        assert_eq!(val, 0x11223344);
        assert_eq!(bus.vdp.control_address, 4);
    }
}
