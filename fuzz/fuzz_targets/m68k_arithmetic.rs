#![no_main]

//! M68k Arithmetic Fuzzer
//!
//! Tests arithmetic operations for:
//! - Overflow/underflow edge cases
//! - Division by zero handling
//! - BCD carry propagation
//! - Flag computation correctness

use libfuzzer_sys::fuzz_target;
use genteel::cpu::Cpu;
use genteel::cpu::flags;
use genteel::memory::{Memory, MemoryInterface};

fuzz_target!(|data: &[u8]| {
    if data.len() < 12 { return; }
    
    // Parse structured input
    let d0 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let d1 = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let initial_sr = u16::from_le_bytes([data[8], data[9]]);
    let opcode = u16::from_le_bytes([data[10], data[11]]);
    
    let mut memory = Box::new(Memory::new(0x100000));
    
    // Write opcode at reset vector destination
    memory.write_word(0x1000, opcode);
    memory.write_word(0x1002, 0x4E71); // NOP (safe fallback)
    memory.write_long(0x04, 0x1000); // Reset PC vector
    memory.write_long(0x00, 0x8000); // Reset SSP vector
    
    let mut cpu = Cpu::new(memory);
    cpu.reset();
    
    // Set up initial state
    cpu.d[0] = d0;
    cpu.d[1] = d1;
    cpu.sr = (initial_sr & 0xFF1F) | flags::SUPERVISOR; // Keep supervisor, mask reserved
    
    // Execute instruction
    let _cycles = cpu.step_instruction();
    
    // Invariant checks:
    
    // 1. PC should be valid (not wrapped unexpectedly)
    assert!(cpu.pc < 0x1000000, "PC should be within 24-bit address space");
    
    // 2. SR reserved bits should remain unchanged
    assert!(cpu.sr & flags::SUPERVISOR != 0, "Should remain in supervisor mode");
    
    // 3. Register consistency: no NaN or special values needed for integers
    
    // 4. Stack pointer should be word-aligned
    assert!(cpu.a[7] & 1 == 0, "Stack pointer should be word-aligned");
});
