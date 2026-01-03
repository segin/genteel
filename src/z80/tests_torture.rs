//! Z80 Torture Tests - Extreme Architectural Nuances
//! 
//! These tests verify undocumented and cycle-accurate behaviors:
//! 1. IM 2 Vector Fetching (Bus Interaction)
//! 2. EI Latency (Interrupt Shadowing)
//! 3. R Register (Bit 7 Preservation & 7-bit wrap)
//! 4. MEMPTR (WZ) State Leakage (BIT flags)
//! 5. Block instruction flag edge cases

use super::*;
use crate::memory::Memory;

fn z80(program: &[u8]) -> Z80 {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() { m.data[i] = b; }
    Z80::new(m)
}

// ============ 1. IM 2 Vector Fetching ============
#[test]
fn torture_im2_vector_fetch() {
    let mut c = z80(&[0xED, 0x5E]); // IM 2
    c.i = 0x10;
    c.iff1 = true;
    c.step(); // IM 2
    
    // Setup vector table at 0x10FF
    // Vector provided by bus is 0xFF
    c.memory.data[0x10FF] = 0x30;
    c.memory.data[0x1100] = 0x20; 
    
    c.trigger_interrupt(0xFF);
    
    assert_eq!(c.pc, 0x2030); // Jumped to handler!
    assert!(!c.iff1); // Interrupts disabled
}

// ============ 2. EI Latency (Interrupt Shadow) ============
#[test]
fn torture_ei_latency_shadow() {
    let mut c = z80(&[0xFB, 0x3C, 0x3C]); // EI; INC A; INC A
    c.iff1 = false;
    c.step(); // EI
    assert!(c.iff1);
    assert!(c.pending_ei); // Internal shadow flag set
    
    // Attempt interrupt during shadow: MUST FAIL (return 0 cycles)
    let cycles = c.trigger_interrupt(0xFF);
    assert_eq!(cycles, 0);
    assert_eq!(c.pc, 0x0001); // Still at first INC A
    
    // After one instruction, shadow is gone
    c.step(); // INC A
    assert!(!c.pending_ei);
    
    // Now interrupt should fire
    c.memory.data[0x0038] = 0x00; // Handler at 0x0038 is NOP
    let cycles = c.trigger_interrupt(0x00);
    assert!(cycles > 0);
    assert_eq!(c.pc, 0x0038);
}

// ============ 3. R Register Nuances ============
#[test]
fn torture_r_reg_bit7_preservation() {
    let mut c = z80(&[0x00, 0x00]); // NOP; NOP
    c.r = 0x80; // Bit 7 set
    c.step(); // Fetch NOP (increments R)
    assert_eq!(c.r & 0x80, 0x80); // Bit 7 MUST remain set
    assert_eq!(c.r & 0x7F, 0x01); // Lower 7 bits increment
}

#[test]
fn torture_r_reg_7bit_wrap() {
    let mut c = z80(&[0x00]); 
    c.r = 0x7F; // Max for lower 7 bits
    c.step();
    assert_eq!(c.r & 0x7F, 0x00); // Should wrap to 0, not 0x80
}

// ============ 4. MEMPTR (WZ) State Leakage ============
#[test]
fn torture_bit_hl_memptr_leakage() {
    let mut c = z80(&[0xCB, 0x46]); // BIT 0, (HL)
    c.memptr = 0x2800; // Bit 5 & 3 of high byte are 1
    c.set_hl(0x8000);
    c.memory.data[0x8000] = 0x00;
    c.step();
    assert!(c.get_flag(flags::X_FLAG)); 
    assert!(c.get_flag(flags::Y_FLAG)); 
}

#[test]
fn torture_bit_ix_ea_leakage() {
    // BIT 0, (IX+0x28)
    // EA = IX + 0x28. If high byte of EA is 0x28, X/Y flags come from bits 3/5 of 0x28.
    // 0x28 = 0010 1000. Bit 5=1, Bit 3=1.
    let mut c = z80(&[0xDD, 0xCB, 0x28, 0x46]); 
    c.ix = 0x2800; // EA = 0x2800 + 0x28 = 0x2828. High byte is 0x28.
    c.memory.data[0x2828] = 0x00;
    c.step(); 
    assert!(c.get_flag(flags::X_FLAG));
    assert!(c.get_flag(flags::Y_FLAG));
}

// ============ 5. Block Instruction Flags ============
#[test]
fn torture_ldi_flags() {
    let mut c = z80(&[0xED, 0xA0]); // LDI
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0002);
    c.memory.data[0x1000] = 0x55;
    c.step();
    // Flags: N=0, H=0, PV=1 (BC != 0), Z=0, C preserved
    assert!(!c.get_flag(flags::ADD_SUB));
    assert!(!c.get_flag(flags::HALF_CARRY));
    assert!(c.get_flag(flags::PARITY)); // PV=1 because BC=1 after dec
}
