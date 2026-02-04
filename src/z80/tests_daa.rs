//! Z80 DAA (Decimal Adjust Accumulator) Tests
//!
//! DAA is one of the most complex Z80 instructions to implement correctly.
//! It adjusts A to valid BCD after ADD or SUB operations.

use super::*;
use crate::memory::Memory;

fn z80(program: &[u8]) -> Z80 {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() { m.data[i] = b; }
    Z80::new(Box::new(m), Box::new(crate::z80::test_utils::TestIo::default()))
}

// ============ DAA after ADD (N=0) - no carries ============

#[test] fn daa_00_add() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.step(); assert_eq!(c.a, 0x00); }
#[test] fn daa_09_add() { let mut c = z80(&[0x27]); c.a = 0x09; c.f = 0; c.step(); assert_eq!(c.a, 0x09); }
#[test] fn daa_0a_add() { let mut c = z80(&[0x27]); c.a = 0x0A; c.f = 0; c.step(); assert_eq!(c.a, 0x10); }
#[test] fn daa_0f_add() { let mut c = z80(&[0x27]); c.a = 0x0F; c.f = 0; c.step(); assert_eq!(c.a, 0x15); }
#[test] fn daa_10_add() { let mut c = z80(&[0x27]); c.a = 0x10; c.f = 0; c.step(); assert_eq!(c.a, 0x10); }
#[test] fn daa_19_add() { let mut c = z80(&[0x27]); c.a = 0x19; c.f = 0; c.step(); assert_eq!(c.a, 0x19); }
#[test] fn daa_1a_add() { let mut c = z80(&[0x27]); c.a = 0x1A; c.f = 0; c.step(); assert_eq!(c.a, 0x20); }
#[test] fn daa_90_add() { let mut c = z80(&[0x27]); c.a = 0x90; c.f = 0; c.step(); assert_eq!(c.a, 0x90); }
#[test] fn daa_99_add() { let mut c = z80(&[0x27]); c.a = 0x99; c.f = 0; c.step(); assert_eq!(c.a, 0x99); }
#[test] fn daa_9a_add() { let mut c = z80(&[0x27]); c.a = 0x9A; c.f = 0; c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_a0_add() { let mut c = z80(&[0x27]); c.a = 0xA0; c.f = 0; c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_ff_add() { let mut c = z80(&[0x27]); c.a = 0xFF; c.f = 0; c.step(); assert_eq!(c.a, 0x65); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after ADD with Half-Carry ============

#[test] fn daa_00_add_h() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x06); }
#[test] fn daa_09_add_h() { let mut c = z80(&[0x27]); c.a = 0x09; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x0F); }
#[test] fn daa_0a_add_h() { let mut c = z80(&[0x27]); c.a = 0x0A; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x10); }
#[test] fn daa_90_add_h() { let mut c = z80(&[0x27]); c.a = 0x90; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x96); }
#[test] fn daa_9a_add_h() { let mut c = z80(&[0x27]); c.a = 0x9A; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after ADD with Carry ============

#[test] fn daa_00_add_c() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x60); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_09_add_c() { let mut c = z80(&[0x27]); c.a = 0x09; c.f = 0; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x69); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_0a_add_c() { let mut c = z80(&[0x27]); c.a = 0x0A; c.f = 0; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x70); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_90_add_c() { let mut c = z80(&[0x27]); c.a = 0x90; c.f = 0; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0xF0); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after ADD with H+C ============

#[test] fn daa_00_add_hc() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x66); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_99_add_hc() { let mut c = z80(&[0x27]); c.a = 0x99; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0xFF); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after SUB (N=1) ============

#[test] fn daa_00_sub() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.step(); assert_eq!(c.a, 0x00); }
#[test] fn daa_09_sub() { let mut c = z80(&[0x27]); c.a = 0x09; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.step(); assert_eq!(c.a, 0x09); }
#[test] fn daa_10_sub() { let mut c = z80(&[0x27]); c.a = 0x10; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.step(); assert_eq!(c.a, 0x10); }
#[test] fn daa_99_sub() { let mut c = z80(&[0x27]); c.a = 0x99; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.step(); assert_eq!(c.a, 0x99); }

// ============ DAA after SUB with Half-Carry ============

#[test] fn daa_00_sub_h() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0xFA); }
#[test] fn daa_10_sub_h() { let mut c = z80(&[0x27]); c.a = 0x10; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x0A); }
#[test] fn daa_ff_sub_h() { let mut c = z80(&[0x27]); c.a = 0xFF; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0xF9); assert!(!c.get_flag(flags::CARRY)); }

// ============ DAA after SUB with Carry ============

#[test] fn daa_00_sub_c() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0xA0); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_60_sub_c() { let mut c = z80(&[0x27]); c.a = 0x60; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after SUB with H+C ============

#[test] fn daa_00_sub_hc() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x9A); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_66_sub_hc() { let mut c = z80(&[0x27]); c.a = 0x66; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA flag behavior ============

#[test] fn daa_zero_flag() { let mut c = z80(&[0x27]); c.a = 0x9A; c.f = 0; c.step(); assert!(c.get_flag(flags::ZERO)); }
#[test] fn daa_sign_flag() { let mut c = z80(&[0x27]); c.a = 0x80; c.f = 0; c.step(); assert!(c.get_flag(flags::SIGN)); }
#[test] fn daa_parity_flag() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.step(); assert!(c.get_flag(flags::PARITY)); } // 0 has even parity

// ============ Real BCD operations: 99 + 1 = 100 (carry) ============

#[test] fn daa_bcd_99_plus_1() {
    // Simulate 99 + 01 in BCD
    let mut c = z80(&[0x80, 0x27]); // ADD A, B; DAA
    c.a = 0x99;
    c.b = 0x01;
    c.step(); // ADD
    c.step(); // DAA
    assert_eq!(c.a, 0x00);
    assert!(c.get_flag(flags::CARRY)); // Overflow to 100
}

#[test] fn daa_bcd_45_plus_37() {
    // 45 + 37 = 82 in BCD
    let mut c = z80(&[0x80, 0x27]);
    c.a = 0x45;
    c.b = 0x37;
    c.step();
    c.step();
    assert_eq!(c.a, 0x82);
    assert!(!c.get_flag(flags::CARRY));
}

#[test] fn daa_bcd_50_minus_25() {
    // 50 - 25 = 25 in BCD
    let mut c = z80(&[0x90, 0x27]); // SUB B; DAA
    c.a = 0x50;
    c.b = 0x25;
    c.step();
    c.step();
    assert_eq!(c.a, 0x25);
}

#[test] fn daa_bcd_25_minus_50() {
    // 25 - 50 = -25, represented as 75 with borrow in BCD
    let mut c = z80(&[0x90, 0x27]);
    c.a = 0x25;
    c.b = 0x50;
    c.step();
    c.step();
    assert_eq!(c.a, 0x75);
    assert!(c.get_flag(flags::CARRY)); // Borrow
}

#[test] fn daa_pc() { let mut c = z80(&[0x27]); c.a = 0; c.step(); assert_eq!(c.pc, 1); }

#[test]

fn daa_full_state_space() {
    // Reference DAA implementation based on Z80 documented behavior
    fn _reference_daa(a: u8, flags: u8) -> (u8, u8) {
        let n = (flags & flags::ADD_SUB as u8) != 0;
        let c = (flags & flags::CARRY as u8) != 0;
        let h = (flags & flags::HALF_CARRY as u8) != 0;
        
        let mut val = a;
        let mut new_c = c;
        let mut diff = 0;
        
        // Determine correction factor
        if !n { // ADD
            if h || (val & 0x0F) > 9 { diff |= 0x06; }
            if c || val > 0x99 { diff |= 0x60; new_c = true; }
            // Special rule: if (val & 0x0F) > 9 was the only reason for correction,
            // check high digit transition? 
            // Simplified logic table:
            // Input C | High>9 | H | Low>9 -> Add
            // 0       | 0      | 0 | 0     -> 00
            // 0       | 0      | 0 | 1     -> 06
            // 0       | 0      | 1 | 0     -> 06 (invalid BCD but H set)
            // ...
            // The conditions above cover it:
            // High nibble adjustment depends on C or (A > 0x99)
            // But wait, A > 0x99 covers High > 9 for valid low inputs.
            // What if High=9 and Low=A (A=9A)? > 0x99 is false.
            // Zilog table says: A-F in upper implies +60.
            // Let's refine the ADD logic to strictly follow the patterns.
            if c || (val > 0x99) { 
                 diff |= 0x60; 
                 new_c = true; 
            }
            if (a & 0x0F) > 9 || h {
                 diff |= 0x06;
                 // Does this affect C? No.
            }
        } else { // SUB
            if c { diff |= 0x60; new_c = true; }
            if h { diff |= 0x06; }
        }
        
        // Apply correction
        val = if !n { val.wrapping_add(diff) } else { val.wrapping_sub(diff) };
        
        // Calculate new flags
        let mut new_flags = flags;
        
        // C
        if new_c { new_flags |= flags::CARRY as u8; } else { new_flags &= !(flags::CARRY as u8); }
        
        // H - Parity? Z80 DAA sets H differently.
        // Actually, for Z80 DAA:
        // H is "Unpredictable" / depends on diff?
        // Wait, different docs say different things.
        // Zilog Manual: "H is set if a BCD carry/borrow occurred from bit 4"
        // Let's rely on the emulator implementation to be "self-consistent" for now?
        // No, the test must be authoritative.
        // Standard behavior: H = (A_before.bit4 ^ A_after.bit4) ? No.
        // Let's simplify: Test only A and C for correctness first. 
        // Zero, Sign, Parity are set from result. H is complex.
        // We will assert A and C match essentially.
        
        // S, Z, P are standard calc on result
        if (val & 0x80) != 0 { new_flags |= flags::SIGN as u8; } else { new_flags &= !(flags::SIGN as u8); }
        if val == 0 { new_flags |= flags::ZERO as u8; } else { new_flags &= !(flags::ZERO as u8); }
        
        // Parity is P/V parity of result
        let p = val.count_ones() % 2 == 0;
        if p { new_flags |= flags::PARITY as u8; } else { new_flags &= !(flags::PARITY as u8); }
        
        (val, new_flags)
    }

    // Iterate all states
    let mut errors = 0;
    for a in 0..=255 {
        for f in 0..=255 {
            let a_in = a as u8;
            let f_in = f as u8;
            
            let mut c = z80(&[0x27]); // DAA
            c.a = a_in;
            c.f = f_in;
            c.step();
            
            // For now, assert that we produce "some" result that doesn't panic.
            // But strict checking:
            // We'll rely on our reference logic approximation for C and Result.
            // Since our Reference logic above was "approximate", let's use the explicit logic from the Z80 impl itself?
            // No, that's tautological.
            // Use the "Algorithmic Definition" widely accepted:
            // 1. If lower nibble > 9 or H=1, diff += 0x06
            // 2. If upper nibble > 9 or C=1 or (upper=9 and lower>9), diff += 0x60, Set C=1
            //    Wait, "upper=9 and lower>9" is "Val > 0x99".
            
            // Let's refine the ref logic:
            let n = (f_in & flags::ADD_SUB as u8) != 0;
            let c_flag = (f_in & flags::CARRY as u8) != 0;
            let h_flag = (f_in & flags::HALF_CARRY as u8) != 0;
            
            let expected_a;
            let mut expected_c = c_flag;
            
            let mut diff = 0;
            if !n { // ADD
                if h_flag || (a_in & 0x0F) > 9 { diff |= 0x06; }
                if c_flag || a_in > 0x99 { diff |= 0x60; expected_c = true; }
            } else { // SUB
                 if h_flag { diff |= 0x06; }
                 if c_flag { diff |= 0x60; expected_c = true; }
            }
            
            expected_a = if !n { a_in.wrapping_add(diff) } else { a_in.wrapping_sub(diff) };
            
            // Assertions
            if c.a != expected_a {
                // If it fails, print details
                // Only print first few errors
                if errors < 10 {
                    println!("DAA mismatch A: input A={:02x} F={:08b} | Exp={:02x} Got={:02x}", a_in, f_in, expected_a, c.a);
                }
                errors += 1;
            }
            if c.get_flag(flags::CARRY) != expected_c {
                if errors < 10 {
                   println!("DAA mismatch C: input A={:02x} F={:08b} | ExpC={} GotC={}", a_in, f_in, expected_c, c.get_flag(flags::CARRY));
                }
                errors += 1;
            }
        }
    }
    assert_eq!(errors, 0, "DAA exhaustive test failed with {} errors", errors);
}
