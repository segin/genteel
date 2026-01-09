#![no_main]

//! M68k Random Execution Fuzzer
//!
//! Full random opcode execution with comprehensive invariant checks.
//! Designed to catch UB, panics, and logic errors.

use libfuzzer_sys::fuzz_target;
use genteel::cpu::Cpu;
use genteel::cpu::flags;
use genteel::memory::{Memory, MemoryInterface};

fuzz_target!(|data: &[u8]| {
    if data.len() < 32 { return; }
    
    let mut memory = Box::new(Memory::new(0x100000));
    
    // Initialize from fuzz data
    let d_regs: [u32; 8] = [
        u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
        u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
        u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
        0, 0, 0, 0,
    ];
    
    // Copy remaining data as program
    let program_start = 16;
    let program = &data[program_start..];
    let copy_len = std::cmp::min(program.len(), 0x200);
    for (i, &b) in program[..copy_len].iter().enumerate() {
        memory.write_byte(0x1000 + i as u32, b);
    }
    
    // Set up exception vectors
    for vector in 0..64 {
        memory.write_long(vector * 4, 0x3000);
    }
    memory.write_word(0x3000, 0x4E72); // STOP
    memory.write_word(0x3002, 0x2700);
    
    // Reset vectors
    memory.write_long(0x00, 0x8000);
    memory.write_long(0x04, 0x1000);
    
    let mut cpu = Cpu::new(memory);
    cpu.reset();
    
    // Initialize data registers
    for i in 0..4 {
        cpu.d[i] = d_regs[i];
    }
    
    // Set up safe address registers
    cpu.a[0] = 0x4000;
    cpu.a[1] = 0x5000;
    cpu.a[2] = 0x6000;
    cpu.a[3] = 0x7000;
    
    // Execute with limits
    let mut total_cycles = 0u64;
    
    for step in 0..100 {
        if cpu.halted { break; }
        if cpu.pc >= 0x1000 + copy_len as u32 { break; }
        if cpu.pc < 0x1000 { break; } // Jumped outside program
        
        let cycles = cpu.step_instruction();
        total_cycles += cycles as u64;
        
        // Cycle count sanity
        assert!(cycles >= 4 && cycles <= 200, 
            "Unreasonable cycle count {} at step {}", cycles, step);
        
        // PC sanity
        assert!(cpu.pc < 0x1000000, "PC out of bounds");
        
        // SP alignment
        assert!(cpu.a[7] & 1 == 0, "Stack misaligned");
        
        // Limit total cycles
        if total_cycles > 10000 { break; }
    }
});
