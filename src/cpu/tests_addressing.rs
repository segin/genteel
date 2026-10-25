//! Effective-address resolution: address results, register side effects
//! and the bus/internal cycles each mode costs.

use crate::cpu::addressing::EffectiveAddress;
use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::test_utils::create_cpu;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};
use proptest::prelude::*;

/// Resolve `mode` with PC at 0x1002 (extension words follow) and return the
/// EA, the cycles spent and the program fetches issued.
fn resolve(
    cpu: &mut Cpu,
    memory: &mut Memory,
    mode: AddressingMode,
    size: Size,
) -> (EffectiveAddress, u32, u32) {
    cpu.pc = 0x1002;
    cpu.icycles = 0;
    cpu.fetches = 0;
    let ea = cpu.calc_ea(mode, size, memory);
    (ea, cpu.icycles, cpu.fetches)
}

#[test]
fn register_modes_cost_nothing() {
    let (mut cpu, mut memory) = create_cpu();
    for r in 0..8 {
        let (ea, cyc, _) = resolve(
            &mut cpu,
            &mut memory,
            AddressingMode::DataRegister(r),
            Size::Long,
        );
        assert_eq!(ea, EffectiveAddress::DataRegister(r));
        assert_eq!(cyc, 0);
        let (ea, cyc, _) = resolve(
            &mut cpu,
            &mut memory,
            AddressingMode::AddressRegister(r),
            Size::Long,
        );
        assert_eq!(ea, EffectiveAddress::AddressRegister(r));
        assert_eq!(cyc, 0);
    }
}

#[test]
fn postincrement_and_predecrement_step_by_size() {
    let (mut cpu, mut memory) = create_cpu();
    for (size, step) in [(Size::Byte, 1), (Size::Word, 2), (Size::Long, 4)] {
        cpu.a[2] = 0x2000;
        let (ea, cyc, _) = resolve(
            &mut cpu,
            &mut memory,
            AddressingMode::AddressPostIncrement(2),
            size,
        );
        assert_eq!(ea, EffectiveAddress::Memory(0x2000));
        assert_eq!(cpu.a[2], 0x2000 + step);
        assert_eq!(cyc, 0);

        cpu.a[2] = 0x2000;
        let (ea, cyc, _) = resolve(
            &mut cpu,
            &mut memory,
            AddressingMode::AddressPreDecrement(2),
            size,
        );
        assert_eq!(ea, EffectiveAddress::Memory(0x2000 - step));
        assert_eq!(cpu.a[2], 0x2000 - step);
        assert_eq!(cyc, 2, "predecrement spends 2 internal clocks");
    }
}

#[test]
fn byte_stack_pointer_steps_by_two() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.a[7] = 0x3000;
    resolve(
        &mut cpu,
        &mut memory,
        AddressingMode::AddressPostIncrement(7),
        Size::Byte,
    );
    assert_eq!(cpu.a[7], 0x3002);
    resolve(
        &mut cpu,
        &mut memory,
        AddressingMode::AddressPreDecrement(7),
        Size::Byte,
    );
    assert_eq!(cpu.a[7], 0x3000);
}

#[test]
fn displacement_and_index_modes() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.a[1] = 0x4000;
    memory.write_word(0x1002, 0xFFFE); // d16 = -2
    let (ea, cyc, fetches) = resolve(
        &mut cpu,
        &mut memory,
        AddressingMode::AddressDisplacement(1),
        Size::Word,
    );
    assert_eq!(ea, EffectiveAddress::Memory(0x3FFE));
    assert_eq!((cyc, fetches), (4, 1));

    /* d8(A1,D3.W): D3.W = -1, d8 = 4 */
    cpu.d[3] = 0x1234_FFFF;
    memory.write_word(0x1002, 0x3004);
    let (ea, cyc, fetches) = resolve(
        &mut cpu,
        &mut memory,
        AddressingMode::AddressIndex(1),
        Size::Word,
    );
    assert_eq!(ea, EffectiveAddress::Memory(0x4003));
    assert_eq!((cyc, fetches), (6, 1));

    /* d8(PC,A2.L): base is the extension word's address */
    cpu.a[2] = 0x10;
    memory.write_word(0x1002, 0xA800);
    let (ea, cyc, _) = resolve(&mut cpu, &mut memory, AddressingMode::PcIndex, Size::Word);
    assert_eq!(ea, EffectiveAddress::Memory(0x1012));
    assert_eq!(cyc, 6);
}

#[test]
fn absolute_and_immediate_modes() {
    let (mut cpu, mut memory) = create_cpu();
    memory.write_word(0x1002, 0x8000);
    let (ea, cyc, _) = resolve(
        &mut cpu,
        &mut memory,
        AddressingMode::AbsoluteShort,
        Size::Word,
    );
    assert_eq!(
        ea,
        EffectiveAddress::Memory(0xFFFF_8000),
        "abs.W sign-extends"
    );
    assert_eq!(cyc, 4);

    memory.write_long(0x1002, 0x00FF_1234);
    let (ea, cyc, _) = resolve(
        &mut cpu,
        &mut memory,
        AddressingMode::AbsoluteLong,
        Size::Word,
    );
    assert_eq!(ea, EffectiveAddress::Memory(0x00FF_1234));
    assert_eq!(cyc, 8);

    memory.write_word(0x1002, 0xAB12);
    let (ea, _, _) = resolve(&mut cpu, &mut memory, AddressingMode::Immediate, Size::Byte);
    assert_eq!(ea, EffectiveAddress::Immediate(0x12));
    memory.write_long(0x1002, 0xDEAD_BEEF);
    let (ea, cyc, _) = resolve(&mut cpu, &mut memory, AddressingMode::Immediate, Size::Long);
    assert_eq!(ea, EffectiveAddress::Immediate(0xDEAD_BEEF));
    assert_eq!(cyc, 8);
}

#[test]
fn long_operands_cost_four_more_clocks_than_word() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.a[0] = 0x2000;
    let ea = EffectiveAddress::Memory(0x2000);
    cpu.icycles = 0;
    cpu.read_operand(ea, Size::Word, &mut memory).unwrap();
    let word = cpu.icycles;
    cpu.icycles = 0;
    cpu.read_operand(ea, Size::Long, &mut memory).unwrap();
    assert_eq!(cpu.icycles, word + 4);
}

#[test]
fn odd_word_access_faults_without_touching_memory() {
    let (mut cpu, mut memory) = create_cpu();
    memory.write_word(0x2000, 0x1234);
    let fault = cpu
        .write_operand(
            EffectiveAddress::Memory(0x2001),
            Size::Word,
            0xFFFF,
            &mut memory,
        )
        .unwrap_err();
    assert_eq!(fault.addr, 0x2001);
    assert!(fault.write && !fault.program);
    assert_eq!(memory.read_word(0x2000), 0x1234);
    assert!(cpu
        .read_operand(EffectiveAddress::Memory(0x2001), Size::Byte, &mut memory)
        .is_ok());
}

proptest! {
    /* Address arithmetic wraps at 32 bits for any base and displacement. */
    #[test]
    fn displacement_wraps(base in any::<u32>(), disp in any::<i16>()) {
        let (mut cpu, mut memory) = create_cpu();
        cpu.a[4] = base;
        memory.write_word(0x1002, disp as u16);
        let (ea, _, _) = resolve(&mut cpu, &mut memory, AddressingMode::AddressDisplacement(4), Size::Word);
        prop_assert_eq!(ea, EffectiveAddress::Memory(base.wrapping_add(disp as i32 as u32)));
    }

    #[test]
    fn index_wraps(base in any::<u32>(), index in any::<u32>(), d8 in any::<i8>(), long in any::<bool>()) {
        let (mut cpu, mut memory) = create_cpu();
        cpu.a[4] = base;
        cpu.d[5] = index;
        memory.write_word(0x1002, 0x5000 | if long { 0x0800 } else { 0 } | d8 as u8 as u16);
        let (ea, _, _) = resolve(&mut cpu, &mut memory, AddressingMode::AddressIndex(4), Size::Word);
        let idx = if long { index } else { index as u16 as i16 as i32 as u32 };
        prop_assert_eq!(ea, EffectiveAddress::Memory(base.wrapping_add(idx).wrapping_add(d8 as i32 as u32)));
    }
}
