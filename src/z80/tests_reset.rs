use crate::z80::Z80;
use crate::memory::Memory;
use crate::z80::test_utils::TestIo;

#[test]
fn test_reset() {
    let memory = Memory::new(0x10000);
    let mut z80 = Z80::new(
        Box::new(memory),
        Box::new(TestIo::default()),
    );

    // Modify state to non-reset values
    z80.a = 0x00;
    z80.f = 0x00;
    z80.b = 0x12;
    z80.c = 0x34;
    z80.d = 0x56;
    z80.e = 0x78;
    z80.h = 0x9A;
    z80.l = 0xBC;

    z80.a_prime = 0x11;
    z80.f_prime = 0x22;
    z80.b_prime = 0x33;
    z80.c_prime = 0x44;
    z80.d_prime = 0x55;
    z80.e_prime = 0x66;
    z80.h_prime = 0x77;
    z80.l_prime = 0x88;

    z80.ix = 0x1122;
    z80.iy = 0x3344;
    z80.sp = 0x1000;
    z80.pc = 0x2000;

    z80.i = 0x55;
    z80.r = 0x66;

    z80.iff1 = true;
    z80.iff2 = true;
    z80.im = 2;
    z80.memptr = 0xDEAD;
    z80.halted = true;
    z80.pending_ei = true;

    // Perform reset
    z80.reset();

    // Verify reset state
    assert_eq!(z80.a, 0xFF, "A register should be 0xFF after reset");
    assert_eq!(z80.f, 0xFF, "F register should be 0xFF after reset");
    assert_eq!(z80.pc, 0x0000, "PC should be 0 after reset");
    assert_eq!(z80.sp, 0xFFFF, "SP should be 0xFFFF after reset");
    assert_eq!(z80.i, 0, "I register should be 0 after reset");
    assert_eq!(z80.r, 0, "R register should be 0 after reset");
    assert_eq!(z80.iff1, false, "IFF1 should be false after reset");
    assert_eq!(z80.iff2, false, "IFF2 should be false after reset");
    assert_eq!(z80.im, 0, "IM should be 0 after reset");
    assert_eq!(z80.memptr, 0, "MEMPTR should be 0 after reset");
    assert_eq!(z80.halted, false, "Halted should be false after reset");
    assert_eq!(z80.pending_ei, false, "Pending EI should be false after reset");

    // Verify preserved state (general purpose registers are NOT reset)
    assert_eq!(z80.b, 0x12, "B register should be preserved");
    assert_eq!(z80.c, 0x34, "C register should be preserved");
    assert_eq!(z80.d, 0x56, "D register should be preserved");
    assert_eq!(z80.e, 0x78, "E register should be preserved");
    assert_eq!(z80.h, 0x9A, "H register should be preserved");
    assert_eq!(z80.l, 0xBC, "L register should be preserved");

    assert_eq!(z80.a_prime, 0x11, "A' register should be preserved");
    assert_eq!(z80.f_prime, 0x22, "F' register should be preserved");
    assert_eq!(z80.b_prime, 0x33, "B' register should be preserved");
    assert_eq!(z80.c_prime, 0x44, "C' register should be preserved");
    assert_eq!(z80.d_prime, 0x55, "D' register should be preserved");
    assert_eq!(z80.e_prime, 0x66, "E' register should be preserved");
    assert_eq!(z80.h_prime, 0x77, "H' register should be preserved");
    assert_eq!(z80.l_prime, 0x88, "L' register should be preserved");

    assert_eq!(z80.ix, 0x1122, "IX register should be preserved");
    assert_eq!(z80.iy, 0x3344, "IY register should be preserved");
}
