use super::*;
use crate::memory::Memory;
use crate::z80::test_utils::TestIo;
use serde_json::json;

fn create_z80() -> Z80<Memory, TestIo> {
    let memory = Memory::new(0x10000);
    Z80::new(memory, TestIo::default())
}

#[test]
fn test_read_write_state() {
    let mut z80 = create_z80();
    z80.a = 0xAA;
    z80.f = 0xBB;
    z80.bc(); // just to use it
    z80.set_bc(0x1234);
    z80.set_de(0x5678);
    z80.set_hl(0x9ABC);
    z80.pc = 0xDEAD;
    z80.sp = 0xBEEF;
    z80.ix = 0x1111;
    z80.iy = 0x2222;
    z80.halted = true;
    z80.im = 2;
    z80.iff1 = true;
    z80.iff2 = false;
    z80.cycles = 123456;

    let state = z80.read_state();

    // Verify read
    assert_eq!(state["a"], 0xAA);
    assert_eq!(state["f"], 0xBB);
    assert_eq!(state["b"], 0x12);
    assert_eq!(state["c"], 0x34);
    assert_eq!(state["pc"], 0xDEAD);
    assert_eq!(state["halted"], true);

    // Verify partial write
    let mut z80_new = create_z80();
    let partial_state = json!({
        "a": 0xFF,
        "pc": 0x0000
    });
    z80_new.write_state(&partial_state);

    assert_eq!(z80_new.a, 0xFF);
    assert_eq!(z80_new.pc, 0x0000);
    // Should be default
    assert_eq!(z80_new.b, 0x00);

    // Verify full write
    z80_new.write_state(&state);
    assert_eq!(z80_new.a, 0xAA);
    assert_eq!(z80_new.f, 0xBB);
    assert_eq!(z80_new.b, 0x12);
    assert_eq!(z80_new.c, 0x34);
    assert_eq!(z80_new.d, 0x56);
    assert_eq!(z80_new.e, 0x78);
    assert_eq!(z80_new.h, 0x9A);
    assert_eq!(z80_new.l, 0xBC);
    assert_eq!(z80_new.pc, 0xDEAD);
    assert_eq!(z80_new.sp, 0xBEEF);
    assert_eq!(z80_new.ix, 0x1111);
    assert_eq!(z80_new.iy, 0x2222);
    assert_eq!(z80_new.halted, true);
    assert_eq!(z80_new.im, 2);
    assert_eq!(z80_new.iff1, true);
    assert_eq!(z80_new.iff2, false);
    assert_eq!(z80_new.cycles, 123456);
}
