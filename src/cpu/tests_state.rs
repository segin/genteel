#![cfg(test)]

use crate::cpu::{CpuState, flags};
use crate::cpu::test_utils::create_test_cpu;

#[test]
fn test_set_state() {
    let (mut cpu, _memory) = create_test_cpu();

    let state = CpuState {
        d: [1, 2, 3, 4, 5, 6, 7, 8],
        a: [10, 20, 30, 40, 50, 60, 70, 80],
        pc: 0x1234,
        sr: flags::SUPERVISOR | 0x0700, // Supervisor, Mask 7
        halted: true,
        pending_interrupt: 5,
    };

    cpu.set_state(state.clone());

    assert_eq!(cpu.d, state.d);
    assert_eq!(cpu.a, state.a);
    assert_eq!(cpu.pc, state.pc);
    assert_eq!(cpu.sr, state.sr);
    assert_eq!(cpu.halted, state.halted);
    assert_eq!(cpu.pending_interrupt, state.pending_interrupt);
}

#[test]
fn test_get_state() {
    let (mut cpu, _memory) = create_test_cpu();

    cpu.d = [0xAA; 8];
    cpu.a = [0xBB; 8];
    cpu.pc = 0x5555;
    cpu.sr = 0x2000;
    cpu.halted = false;
    cpu.pending_interrupt = 2;

    let state = cpu.get_state();

    assert_eq!(state.d, cpu.d);
    assert_eq!(state.a, cpu.a);
    assert_eq!(state.pc, cpu.pc);
    assert_eq!(state.sr, cpu.sr);
    assert_eq!(state.halted, cpu.halted);
    assert_eq!(state.pending_interrupt, cpu.pending_interrupt);
}

#[test]
fn test_state_roundtrip() {
    let (mut cpu, _memory) = create_test_cpu();

    // Set some random-ish state
    cpu.d = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    cpu.a = [0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8];
    cpu.pc = 0xDEADBEEF;
    cpu.sr = 0x2700;
    cpu.halted = true;
    cpu.pending_interrupt = 7;

    let state1 = cpu.get_state();

    let (mut cpu2, _memory2) = create_test_cpu();
    cpu2.set_state(state1.clone());

    let state2 = cpu2.get_state();

    assert_eq!(state1.d, state2.d);
    assert_eq!(state1.a, state2.a);
    assert_eq!(state1.pc, state2.pc);
    assert_eq!(state1.sr, state2.sr);
    assert_eq!(state1.halted, state2.halted);
    assert_eq!(state1.pending_interrupt, state2.pending_interrupt);
}
