use super::*;
use crate::memory::Memory;

#[cfg(test)]
use crate::z80::test_utils::TestIo;

#[test]
fn test_reset_behavior() {
    let memory = Memory::new(0x1000); // 4KB
    let cpu = Z80::new();
    let mut z80 = crate::z80::test_utils::TestZ80::new(cpu, memory, TestIo::default());

    // 1. Modify fields that SHOULD be reset
    z80.a = 0xAA;
    z80.f = 0x55;
    z80.pc = 0x1234;
    z80.sp = 0x8000;
    z80.i = 0x12;
    z80.r = 0x34;
    z80.iff1 = true;
    z80.iff2 = true;
    z80.im = 2;
    z80.memptr = 0xBEEF;
    z80.halted = true;
    z80.pending_ei = true;

    // 2. Modify fields that SHOULD NOT be reset (based on current implementation)
    z80.b = 0xBB;
    z80.c = 0xCC;
    z80.d = 0xDD;
    z80.e = 0xEE;
    z80.h = 0x11;
    z80.l = 0x22;

    z80.a_prime = 0xA0;
    z80.f_prime = 0xF0;
    z80.b_prime = 0xB0;
    z80.c_prime = 0xC0;
    z80.d_prime = 0xD0;
    z80.e_prime = 0xE0;
    z80.h_prime = 0x10;
    z80.l_prime = 0x20;

    z80.ix = 0x3344;
    z80.iy = 0x5566;

    // 3. Perform Reset
    z80.reset();

    // 4. Verify fields that SHOULD be reset
    assert_eq!(z80.a, 0xFF, "A should be reset to 0xFF");
    assert_eq!(z80.f, 0xFF, "F should be reset to 0xFF");
    assert_eq!(z80.pc, 0, "PC should be reset to 0");
    assert_eq!(z80.sp, 0xFFFF, "SP should be reset to 0xFFFF");
    assert_eq!(z80.i, 0, "I should be reset to 0");
    assert_eq!(z80.r, 0, "R should be reset to 0");
    assert_eq!(z80.iff1, false, "IFF1 should be reset to false");
    assert_eq!(z80.iff2, false, "IFF2 should be reset to false");
    assert_eq!(z80.im, 0, "IM should be reset to 0");
    assert_eq!(z80.memptr, 0, "MEMPTR should be reset to 0");
    assert_eq!(z80.halted, false, "HALTED should be reset to false");
    assert_eq!(z80.pending_ei, false, "PENDING_EI should be reset to false");

    // 5. Verify fields that SHOULD NOT be reset
    assert_eq!(z80.b, 0xBB, "B should not be reset");
    assert_eq!(z80.c, 0xCC, "C should not be reset");
    assert_eq!(z80.d, 0xDD, "D should not be reset");
    assert_eq!(z80.e, 0xEE, "E should not be reset");
    assert_eq!(z80.h, 0x11, "H should not be reset");
    assert_eq!(z80.l, 0x22, "L should not be reset");

    assert_eq!(z80.a_prime, 0xA0, "A' should not be reset");
    assert_eq!(z80.f_prime, 0xF0, "F' should not be reset");
    assert_eq!(z80.b_prime, 0xB0, "B' should not be reset");
    assert_eq!(z80.c_prime, 0xC0, "C' should not be reset");
    assert_eq!(z80.d_prime, 0xD0, "D' should not be reset");
    assert_eq!(z80.e_prime, 0xE0, "E' should not be reset");
    assert_eq!(z80.h_prime, 0x10, "H' should not be reset");
    assert_eq!(z80.l_prime, 0x20, "L' should not be reset");

    assert_eq!(z80.ix, 0x3344, "IX should not be reset");
    assert_eq!(z80.iy, 0x5566, "IY should not be reset");
}
