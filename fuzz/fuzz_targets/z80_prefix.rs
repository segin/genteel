#![no_main]

//! Prefix interaction fuzzer - tests complex prefix sequences
//!
//! The Z80 has interesting prefix behavior:
//! - DD/FD can modify HL to IX/IY
//! - Multiple DD/FD in sequence: only last one applies
//! - DD/FD before ED: ED takes precedence
//! - DD CB / FD CB: indexed bit operations

use libfuzzer_sys::fuzz_target;
use genteel::z80::Z80;
use genteel::memory::Memory;

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 { return; }
    
    // Generate prefix sequences
    let num_prefixes = (data[0] % 4) as usize;
    let prefixes = [0xDD, 0xFD, 0xED, 0xCB];
    
    let mut program = Vec::with_capacity(16);
    for i in 0..num_prefixes {
        if i + 1 < data.len() {
            program.push(prefixes[(data[i + 1] % 4) as usize]);
        }
    }
    
    // Add a base opcode that's safe
    let safe_opcodes = [
        0x00, // NOP
        0x04, // INC B
        0x05, // DEC B
        0x3C, // INC A
        0x3D, // DEC A
        0xAF, // XOR A
        0x78, // LD A, B
    ];
    
    if num_prefixes + 1 < data.len() {
        program.push(safe_opcodes[(data[num_prefixes + 1] % safe_opcodes.len() as u8) as usize]);
    } else {
        program.push(0x00);
    }
    
    // Add operands if needed
    for i in 0..4 {
        if num_prefixes + 2 + i < data.len() {
            program.push(data[num_prefixes + 2 + i]);
        } else {
            program.push(0);
        }
    }
    
    let mut memory = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() {
        memory.data[i] = b;
    }
    
    let mut cpu = Z80::new(memory);
    cpu.sp = 0x8000;
    cpu.set_hl(0x4000);
    cpu.ix = 0x5000;
    cpu.iy = 0x6000;
    cpu.a = data.get(0).copied().unwrap_or(0);
    cpu.b = data.get(1).copied().unwrap_or(0);
    
    // Execute - should not panic
    for _ in 0..10 {
        if cpu.halted { break; }
        if cpu.pc >= program.len() as u16 { break; }
        cpu.step();
    }
    
    // State should be readable
    let _ = cpu.af();
    let _ = cpu.bc();
    let _ = cpu.de();
    let _ = cpu.hl();
});
