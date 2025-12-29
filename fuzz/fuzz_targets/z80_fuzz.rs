#![no_main]

use libfuzzer_sys::fuzz_target;
use genteel::z80::Z80;
use genteel::memory::Memory;

fuzz_target!(|data: &[u8]| {
    // Skip if input is too small
    if data.is_empty() {
        return;
    }

    // Create memory and load fuzz input as a program
    let mut memory = Memory::new(0x10000);
    let copy_len = std::cmp::min(data.len(), 0x8000);
    memory.data[..copy_len].copy_from_slice(&data[..copy_len]);

    // Create Z80 and execute
    let mut z80 = Z80::new(memory);
    z80.sp = 0xFF00; // Set up stack in safe area

    // Execute up to 10000 instructions or until halted
    for _ in 0..10000 {
        if z80.halted {
            break;
        }
        
        // Stop if PC goes past our program
        if z80.pc as usize >= copy_len {
            break;
        }

        z80.step();
    }

    // Verify invariants - these should never panic
    let _ = z80.af();
    let _ = z80.bc();
    let _ = z80.de();
    let _ = z80.hl();
});
