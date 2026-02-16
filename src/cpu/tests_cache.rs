//! Instruction Cache Tests
//!
//! Tests specifically for the instruction cache and self-modifying code.

#![cfg(test)]

use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x10000);
    // Initial SP and PC
    memory.write_long(0, 0x1000); // SP
    memory.write_long(4, 0x100); // PC
    let cpu = Cpu::new(&mut memory);
    (cpu, memory)
}

#[test]
fn test_smc_overwrite_instruction() {
    let (mut cpu, mut memory) = create_cpu();

    // 0x100: MOVE.W #$4E71, $108  (Write NOP to 0x108)
    //        Opcode: 33FC 4E71 0000 0108
    // 0x108: RTS                  (Initial instruction, to be overwritten)
    //        Opcode: 4E75
    // 0x10A: MOVEQ #1, D0         (Should be executed after NOP)
    //        Opcode: 7001

    // Setup initial memory
    // MOVE.W #$4E71, $108
    memory.write_word(0x100, 0x33FC);
    memory.write_word(0x102, 0x4E71); // NOP
    memory.write_long(0x104, 0x00000108);

    // RTS at 0x108
    memory.write_word(0x108, 0x4E75);

    // MOVEQ #1, D0 at 0x10A
    memory.write_word(0x10A, 0x7001);

    // Initial run
    cpu.pc = 0x100;

    // Step 1: Execute MOVE.W #$4E71, $108
    // This writes NOP to 0x108.
    // If cache invalidation works, cache entry for 0x108 is invalidated.
    cpu.step_instruction(&mut memory);

    // Verify memory is updated
    assert_eq!(memory.read_word(0x108), 0x4E71);

    // Step 2: Execute instruction at 0x108
    // If invalidation worked, it fetches NOP (0x4E71).
    // If not, it executes RTS (0x4E75) from cache.
    cpu.step_instruction(&mut memory);

    // If it was NOP, PC should be 0x10A.
    assert_eq!(
        cpu.pc, 0x10A,
        "SMC failed: Executed stale instruction (RTS) instead of new (NOP)"
    );

    // Step 3: Execute MOVEQ #1, D0
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 1);
}
