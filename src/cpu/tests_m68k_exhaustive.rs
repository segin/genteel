//! Exhaustive M68k ALU and Instruction Verification
//!
//! This module implements a "Golden Reference" model for M68k operations
//! and runs exhaustive randomized tests to verify both results and flags (XNZVC).
//! Targeting 3000+ test cases via high-iteration RNG testing.

use super::*;
use crate::cpu::flags;
use crate::cpu::instructions::Size;
use crate::memory::Memory;

// fast rng for exhaustive testing
struct XorShift64 {
    state: u64,
}
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    fn next_u32(&mut self) -> u32 {
        self.next() as u32
    }
}

fn create_test_setup() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x100000);
    // Initialize memory with basic vector table
    memory.write_long(0x0, 0x80000); // Stack pointer
    memory.write_long(0x4, 0x1000); // PC
    let cpu = Cpu::new(&mut memory);
    (cpu, memory)
}

// ============ Reference Models ============

fn ref_add(src: u32, dst: u32, size: Size) -> (u32, u16) {
    let mask = size.mask();
    let msb = size.sign_bit();
    let s = src & mask;
    let d = dst & mask;
    let res = s.wrapping_add(d) & mask;

    let n = (res & msb) != 0;
    let z = res == 0;
    let c = (s as u64 + d as u64) > mask as u64;
    let v = ((s ^ res) & (d ^ res) & msb) != 0;
    let x = c;

    let mut flags = 0u16;
    if n {
        flags |= flags::NEGATIVE;
    }
    if z {
        flags |= flags::ZERO;
    }
    if v {
        flags |= flags::OVERFLOW;
    }
    if c {
        flags |= flags::CARRY;
    }
    if x {
        flags |= flags::EXTEND;
    }
    (res, flags)
}

fn ref_sub(src: u32, dst: u32, size: Size) -> (u32, u16) {
    let mask = size.mask();
    let msb = size.sign_bit();
    let s = src & mask;
    let d = dst & mask;
    let res = d.wrapping_sub(s) & mask;

    let n = (res & msb) != 0;
    let z = res == 0;
    let c = d < s;
    let v = ((d ^ s) & (d ^ res) & msb) != 0;
    let x = c;

    let mut flags = 0u16;
    if n {
        flags |= flags::NEGATIVE;
    }
    if z {
        flags |= flags::ZERO;
    }
    if v {
        flags |= flags::OVERFLOW;
    }
    if c {
        flags |= flags::CARRY;
    }
    if x {
        flags |= flags::EXTEND;
    }
    (res, flags)
}

fn ref_logic(op: &str, src: u32, dst: u32, size: Size, old_x: bool) -> (u32, u16) {
    let mask = size.mask();
    let msb = size.sign_bit();
    let s = src & mask;
    let d = dst & mask;

    let res = match op {
        "AND" => s & d,
        "OR" => s | d,
        "EOR" => s ^ d,
        _ => unreachable!(),
    } & mask;

    let n = (res & msb) != 0;
    let z = res == 0;
    let v = false;
    let c = false;
    let x = old_x;

    let mut flags = 0u16;
    if n {
        flags |= flags::NEGATIVE;
    }
    if z {
        flags |= flags::ZERO;
    }
    if v {
        flags |= flags::OVERFLOW;
    }
    if c {
        flags |= flags::CARRY;
    }
    if x {
        flags |= flags::EXTEND;
    }
    (res, flags)
}

// ============ Exhaustive Tests ============

#[test]
fn exhaustive_m68k_add() {
    let mut rng = XorShift64::new(0x1234567890ABCDEF);
    let (mut cpu, mut memory) = create_test_setup();
    let sizes = [Size::Byte, Size::Word, Size::Long];
    // ADD.x D1, D0 (direction=0, reg=0, opmode=0/1/2, mode=0, ea_reg=1)
    let opcodes = [0xD001, 0xD041, 0xD081];

    for (s_idx, &size) in sizes.iter().enumerate() {
        for i in 0..1000 {
            let a = rng.next_u32(); // D0 (dst)
            let b = rng.next_u32(); // D1 (src)
            cpu.d[0] = a;
            cpu.d[1] = b;
            cpu.sr = 0;
            cpu.pc = 0x1000;
            cpu.write_word(0x1000, opcodes[s_idx], &mut memory);

            cpu.step_instruction(&mut memory);

            // Expected result should preserve high bits of D0
            let (res_part, exp_sr) = ref_add(b, a, size);
            let exp_res = size.apply(a, res_part);

            assert_eq!(
                cpu.d[0], exp_res,
                "ADD.{} iter {}: result mismatch (dst_in={:08X}, src={:08X})",
                size, i, a, b
            );
            assert_eq!(
                cpu.sr & 0x1F,
                exp_sr,
                "ADD.{} iter {}: flags mismatch (dst_in={:08X}, src={:08X})",
                size,
                i,
                a,
                b
            );
        }
    }
}

#[test]
fn exhaustive_m68k_sub() {
    let mut rng = XorShift64::new(0xDEADBEEFCAFEBABE);
    let (mut cpu, mut memory) = create_test_setup();
    let sizes = [Size::Byte, Size::Word, Size::Long];
    // SUB.x D1, D0 (opmode=0/1/2)
    let opcodes = [0x9001, 0x9041, 0x9081];

    for (s_idx, &size) in sizes.iter().enumerate() {
        for i in 0..1000 {
            let a = rng.next_u32(); // D0 (dst)
            let b = rng.next_u32(); // D1 (src)
            cpu.d[0] = a;
            cpu.d[1] = b;
            cpu.sr = 0;
            cpu.pc = 0x1000;
            cpu.write_word(0x1000, opcodes[s_idx], &mut memory);

            cpu.step_instruction(&mut memory);

            // Expected result should preserve high bits of D0
            let (res_part, exp_sr) = ref_sub(b, a, size);
            let exp_res = size.apply(a, res_part);

            assert_eq!(
                cpu.d[0], exp_res,
                "SUB.{} iter {}: result mismatch (dst_in={:08X}, src={:08X})",
                size, i, a, b
            );
            assert_eq!(
                cpu.sr & 0x1F,
                exp_sr,
                "SUB.{} iter {}: flags mismatch (dst_in={:08X}, src={:08X})",
                size,
                i,
                a,
                b
            );
        }
    }
}

#[test]
fn exhaustive_m68k_logic() {
    let mut rng = XorShift64::new(0x9876543210FEDCBA);
    let (mut cpu, mut memory) = create_test_setup();

    // Config: (Name, Opcode, Size)
    let test_configs = [
        ("AND", 0xC081, Size::Long),
        ("OR", 0x8081, Size::Long),
        ("EOR", 0xB181, Size::Long),
    ];

    for (name, opcode, size) in test_configs {
        for i in 0..1000 {
            let a = rng.next_u32(); // Initial D0
            let b = rng.next_u32(); // Initial D1
            let x_init = (rng.next_u32() & 1) != 0;

            cpu.d[0] = a;
            cpu.d[1] = b;
            cpu.sr = if x_init { flags::EXTEND } else { 0 };
            cpu.pc = 0x1000;
            cpu.write_word(0x1000, opcode, &mut memory);

            cpu.step_instruction(&mut memory);

            // For 0xC081 (AND.L D1, D0) -> src=D1, dst=D0
            // For 0x8081 (OR.L D1, D0)  -> src=D1, dst=D0
            // For 0xB181 (EOR.L D0, D1) -> src=D0, dst=D1
            let (exp_res, exp_sr) = if name == "EOR" {
                ref_logic(name, a, b, size, x_init)
            } else {
                ref_logic(name, b, a, size, x_init)
            };

            let actual_res = if name == "EOR" { cpu.d[1] } else { cpu.d[0] };

            assert_eq!(
                actual_res, exp_res,
                "{} iter {}: result mismatch (D0={:08X}, D1={:08X})",
                name, i, a, b
            );
            assert_eq!(
                cpu.sr & 0x1F,
                exp_sr,
                "{} iter {}: flags mismatch (D0={:08X}, D1={:08X})",
                name,
                i,
                a,
                b
            );
        }
    }
}

#[test]
fn torture_m68k_address_error() {
    let (mut cpu, mut memory) = create_test_setup();

    // Set up Address Error Vector (Vector 3 at 0x0C)
    memory.write_long(0x0C, 0x2000);

    // MOVE.W D0, (A0) - Unaligned write
    cpu.write_word(0x1000, 0x3080, &mut memory);
    cpu.a[0] = 0x1001;
    cpu.pc = 0x1000;

    cpu.step_instruction(&mut memory);

    // Should have trapped to 0x2000
    assert_eq!(
        cpu.pc, 0x2000,
        "Should trap to Address Error handler on unaligned access"
    );
    assert!(
        cpu.get_flag(flags::SUPERVISOR),
        "Should be in supervisor mode after trap"
    );
}

#[test]
fn torture_m68k_privilege_violation() {
    let (mut cpu, mut memory) = create_test_setup();

    // Set up Privilege Violation Vector (Vector 8 at 0x20)
    memory.write_long(0x20, 0x3000);

    // MOVE.W D0, SR - Privileged
    cpu.write_word(0x1000, 0x46C0, &mut memory);

    // Switch to User Mode (clear S flag)
    cpu.set_sr(cpu.sr & !flags::SUPERVISOR);
    cpu.pc = 0x1000;

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, 0x3000, "Should trap to Privilege Violation handler");
    assert!(
        cpu.get_flag(flags::SUPERVISOR),
        "Should be in supervisor mode after trap"
    );
}

#[test]
fn torture_m68k_div_by_zero() {
    let (mut cpu, mut memory) = create_test_setup();

    // Set up Zero Divide Vector (Vector 5 at 0x14)
    memory.write_long(0x14, 0x4000);

    // DIVU.W D1, D0
    cpu.write_word(0x1000, 0x80C1, &mut memory);
    cpu.d[0] = 100;
    cpu.d[1] = 0; // Divisor 0
    cpu.pc = 0x1000;

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, 0x4000, "Should trap to Zero Divide handler");
}
