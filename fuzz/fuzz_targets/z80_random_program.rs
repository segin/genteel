#![no_main]

use libfuzzer_sys::fuzz_target;
use genteel::z80::Z80;
use genteel::memory::Memory;

/// More structured fuzzing - interpret input as opcode + operands
fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let mut memory = Memory::new(0x10000);
    
    // Use first bytes to set up initial CPU state
    let initial_a = data.get(0).copied().unwrap_or(0);
    let initial_bc = u16::from_le_bytes([
        data.get(1).copied().unwrap_or(0),
        data.get(2).copied().unwrap_or(0)
    ]);
    
    // Rest is the program
    let program = &data[3..];
    let copy_len = std::cmp::min(program.len(), 0x4000);
    memory.data[..copy_len].copy_from_slice(&program[..copy_len]);

    let mut z80 = Z80::new(memory);
    z80.a = initial_a;
    z80.set_bc(initial_bc);
    z80.sp = 0x8000;
    z80.set_hl(0x2000); // Safe memory area for (HL) operations
    z80.set_de(0x3000);
    z80.ix = 0x4000;
    z80.iy = 0x5000;

    // Execute limited instructions
    for _ in 0..1000 {
        if z80.halted || z80.pc as usize >= copy_len {
            break;
        }
        z80.step();
    }

    // State should always be readable
    assert!(z80.af() <= 0xFFFF);
    assert!(z80.bc() <= 0xFFFF);
    assert!(z80.de() <= 0xFFFF);
    assert!(z80.hl() <= 0xFFFF);
});
