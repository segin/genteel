use crate::cpu::{Cpu, flags};
use crate::memory::{Memory, MemoryInterface};
use std::time::Instant;

#[test]
fn benchmark_interrupt_handler() {
    let mut memory = Memory::new(0x10000);
    // Initial SP and PC
    memory.write_long(0, 0x1000); // SP
    memory.write_long(4, 0x100); // PC

    let mut cpu = Cpu::new(&mut memory);

    // Set Interrupt Mask to 0 to allow Level 6 (Supervisor mode maintained)
    // SR = 0010 0000 0000 0000 = 0x2000
    cpu.sr = 0x2000;

    // Setup Vector 30 (Level 6 Autovector) -> Address 0x78
    // Point to 0x400
    memory.write_long(30 * 4, 0x400);

    // Write RTE at 0x400
    // RTE = 0x4E73
    memory.write_word(0x400, 0x4E73);

    // Write NOP at 0x100 (where PC starts)
    memory.write_word(0x100, 0x4E71);

    let start = Instant::now();
    let iterations = 100_000;

    for _ in 0..iterations {
        // Request VBlank Interrupt (Level 6)
        cpu.request_interrupt(6);

        // 1. Process Interrupt
        // This should consume cycles for exception processing and jump to 0x400
        let cycles_int = cpu.step_instruction(&mut memory);

        // Verify we are at the handler
        assert_eq!(cpu.pc, 0x400, "PC should be at handler (0x400) after interrupt");
        assert!(cycles_int >= 44, "Interrupt processing should take at least 44 cycles");

        // 2. Execute RTE
        // This should pop SR/PC and return to 0x100
        cpu.step_instruction(&mut memory);

        // Verify we returned
        assert_eq!(cpu.pc, 0x100, "PC should return to 0x100 after RTE");

        // Ensure interrupt mask is back to 0 (RTE restored SR)
        assert_eq!(cpu.sr & flags::INTERRUPT_MASK, 0, "SR Interrupt mask should be restored");
    }

    let duration = start.elapsed();
    println!("Benchmark duration: {:?}", duration);

    // Threshold: 500ms. If debug prints were present, this would be >> 1s.
    assert!(duration.as_millis() < 500, "Interrupt handling is too slow! Duration: {:?}", duration);
}
