#![no_main]

//! Memory boundary fuzzer - tests edge cases in memory access
//!
//! Looks for bugs in:
//! - Stack operations near memory boundaries
//! - 16-bit wraparound in address calculations
//! - IX+d / IY+d with extreme displacement values
//! - Block operations (LDI/LDD/LDIR/LDDR)

use libfuzzer_sys::fuzz_target;
use genteel::z80::Z80;
use genteel::memory::Memory;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }
    
    // Test different memory-accessing instructions
    let test_type = data[0] % 6;
    
    let mut memory = Memory::new(0x10000);
    let mut cpu = Z80::new(memory);
    
    match test_type {
        0 => {
            // Stack at boundary
            cpu.memory.data[0] = 0xC5; // PUSH BC
            cpu.memory.data[1] = 0xC1; // POP BC
            cpu.sp = u16::from_le_bytes([data[1], data[2]]);
            cpu.set_bc(u16::from_le_bytes([data[3], data[4]]));
            
            if cpu.sp >= 2 { // Only valid if SP has room
                let orig_bc = cpu.bc();
                cpu.step();
                cpu.set_bc(0);
                cpu.step();
                assert_eq!(cpu.bc(), orig_bc, "PUSH/POP BC failed at SP={:04X}", cpu.sp.wrapping_add(2));
            }
        }
        1 => {
            // IX+d with extreme displacement
            let d = data[1] as i8;
            cpu.memory.data[0] = 0xDD;
            cpu.memory.data[1] = 0x7E; // LD A, (IX+d)
            cpu.memory.data[2] = d as u8;
            cpu.ix = u16::from_le_bytes([data[3], data[4]]);
            
            let addr = (cpu.ix as i32 + d as i32) as u16;
            cpu.memory.data[addr as usize] = 0x42;
            cpu.step();
            assert_eq!(cpu.a, 0x42, "LD A, (IX+{:02X}) failed with IX={:04X}", d, cpu.ix);
        }
        2 => {
            // IY+d with extreme displacement  
            let d = data[1] as i8;
            cpu.memory.data[0] = 0xFD;
            cpu.memory.data[1] = 0x7E; // LD A, (IY+d)
            cpu.memory.data[2] = d as u8;
            cpu.iy = u16::from_le_bytes([data[3], data[4]]);
            
            let addr = (cpu.iy as i32 + d as i32) as u16;
            cpu.memory.data[addr as usize] = 0x55;
            cpu.step();
            assert_eq!(cpu.a, 0x55, "LD A, (IY+{:02X}) failed with IY={:04X}", d, cpu.iy);
        }
        3 => {
            // LDI with various addresses
            cpu.memory.data[0] = 0xED;
            cpu.memory.data[1] = 0xA0; // LDI
            cpu.set_hl(u16::from_le_bytes([data[1], data[2]]));
            cpu.set_de(u16::from_le_bytes([data[3], data[4]]));
            cpu.set_bc(u16::from_le_bytes([data[5], data[6]]).max(1));
            
            let src = cpu.hl();
            let dst = cpu.de();
            let val: u8 = data.get(7).copied().unwrap_or(0xAA);
            cpu.memory.data[src as usize] = val;
            
            cpu.step();
            
            assert_eq!(cpu.memory.data[dst as usize], val, 
                "LDI failed: src={:04X} dst={:04X}", src, dst);
            assert_eq!(cpu.hl(), src.wrapping_add(1), "LDI: HL not incremented");
            assert_eq!(cpu.de(), dst.wrapping_add(1), "LDI: DE not incremented");
        }
        4 => {
            // LDD with various addresses
            cpu.memory.data[0] = 0xED;
            cpu.memory.data[1] = 0xA8; // LDD
            cpu.set_hl(u16::from_le_bytes([data[1], data[2]]));
            cpu.set_de(u16::from_le_bytes([data[3], data[4]]));
            cpu.set_bc(u16::from_le_bytes([data[5], data[6]]).max(1));
            
            let src = cpu.hl();
            let dst = cpu.de();
            let val: u8 = data.get(7).copied().unwrap_or(0xBB);
            cpu.memory.data[src as usize] = val;
            
            cpu.step();
            
            assert_eq!(cpu.memory.data[dst as usize], val,
                "LDD failed: src={:04X} dst={:04X}", src, dst);
            assert_eq!(cpu.hl(), src.wrapping_sub(1), "LDD: HL not decremented");
            assert_eq!(cpu.de(), dst.wrapping_sub(1), "LDD: DE not decremented");
        }
        _ => {
            // LD (nn), HL / LD HL, (nn) with boundary addresses
            let addr = u16::from_le_bytes([data[1], data[2]]);
            if addr < 0xFFFE { // Need 2 bytes
                cpu.memory.data[0] = 0x22; // LD (nn), HL
                cpu.memory.data[1] = (addr & 0xFF) as u8;
                cpu.memory.data[2] = (addr >> 8) as u8;
                cpu.set_hl(u16::from_le_bytes([data[3], data[4]]));
                
                let orig_hl = cpu.hl();
                cpu.step();
                
                let stored = u16::from_le_bytes([
                    cpu.memory.data[addr as usize],
                    cpu.memory.data[addr.wrapping_add(1) as usize]
                ]);
                assert_eq!(stored, orig_hl, "LD (nn), HL failed at addr {:04X}", addr);
            }
        }
    }
});
