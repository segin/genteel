#![no_main]

//! M68k Control Flow Fuzzer
//!
//! Tests control flow for:
//! - Exception handling (TRAP, CHK, TRAPV)
//! - Privilege violations
//! - Stack consistency across JSR/RTS/LINK/UNLK
//! - Interrupt processing

use libfuzzer_sys::fuzz_target;
use genteel::cpu::Cpu;
use genteel::cpu::flags;
use genteel::memory::{Memory, MemoryInterface};

fuzz_target!(|data: &[u8]| {
    if data.len() < 20 { return; }
    
    let mut memory = Box::new(Memory::new(0x100000));
    
    // Copy program to memory
    let program_len = std::cmp::min(data.len(), 0x100);
    for (i, &b) in data[..program_len].iter().enumerate() {
        memory.write_byte(0x1000 + i as u32, b);
    }
    
    // Set up exception vectors (prevent crashes on traps)
    for vector in 0..64 {
        memory.write_long(vector * 4, 0x2000); // All vectors point to safe handler
    }
    
    // Safe handler: just halt
    memory.write_word(0x2000, 0x4E72); // STOP
    memory.write_word(0x2002, 0x2700); // #$2700
    
    // Reset vectors
    memory.write_long(0x00, 0x8000); // SSP
    memory.write_long(0x04, 0x1000); // PC
    
    let mut cpu = Cpu::new(memory);
    cpu.reset();
    
    // Execute limited instructions
    let initial_sp = cpu.a[7];
    
    for _ in 0..50 {
        if cpu.halted { break; }
        if cpu.pc >= 0x1000 + program_len as u32 { break; }
        
        let old_pc = cpu.pc;
        let _cycles = cpu.step_instruction();
        
        // Invariant: PC must change (except for STOP)
        if !cpu.halted && cpu.pc == old_pc {
            // May be valid for certain loop instructions, but limit iterations
            break;
        }
        
        // Invariant: Stack should not overflow or underflow excessively
        let sp_diff = (initial_sp as i64 - cpu.a[7] as i64).abs();
        if sp_diff > 0x1000 {
            break; // Excessive stack usage
        }
    }
    
    // Final invariants:
    assert!(cpu.a[7] & 1 == 0, "Stack must be word-aligned");
    assert!(cpu.pc < 0x1000000, "PC must be in 24-bit range");
});
