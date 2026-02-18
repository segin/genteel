use crate::cpu::addressing::{calculate_ea, EffectiveAddress};
use crate::cpu::decoder::{AddressingMode, Size};
use crate::memory::Memory;
use crate::memory::MemoryInterface;

#[test]
fn test_data_register_direct() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0;
    let mut mem = Memory::new(1024);

    let (ea, cycles) = calculate_ea(
        AddressingMode::DataRegister(3),
        Size::Long,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::DataRegister(reg) => assert_eq!(reg, 3),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(cycles, 0);
}

#[test]
fn test_address_register_direct() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0;
    let mut mem = Memory::new(1024);

    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressRegister(5),
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::AddressRegister(reg) => assert_eq!(reg, 5),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(cycles, 0);
}

#[test]
fn test_address_indirect() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0;
    let mut mem = Memory::new(1024);

    a[2] = 0x100;

    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressIndirect(2),
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x100),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(cycles, 4);
}

#[test]
fn test_address_post_increment() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0;
    let mut mem = Memory::new(1024);

    // Test Byte size (increment by 1)
    a[0] = 0x100;
    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressPostIncrement(0),
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x100),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(a[0], 0x101);
    assert_eq!(cycles, 4);

    // Test Word size (increment by 2)
    a[1] = 0x200;
    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressPostIncrement(1),
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x200),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(a[1], 0x202);
    assert_eq!(cycles, 4);

    // Test Long size (increment by 4)
    a[2] = 0x300;
    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressPostIncrement(2),
        Size::Long,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x300),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(a[2], 0x304);
    assert_eq!(cycles, 4);

    // Test Stack Pointer (A7) with Byte size (should increment by 2)
    a[7] = 0x400;
    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressPostIncrement(7),
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x400),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(a[7], 0x402); // Special case for A7
    assert_eq!(cycles, 4);
}

#[test]
fn test_address_pre_decrement() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0;
    let mut mem = Memory::new(1024);

    // Test Byte size (decrement by 1)
    a[0] = 0x101;
    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressPreDecrement(0),
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x100),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(a[0], 0x100);
    assert_eq!(cycles, 6);

    // Test Word size (decrement by 2)
    a[1] = 0x202;
    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressPreDecrement(1),
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x200),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(a[1], 0x200);
    assert_eq!(cycles, 6);

    // Test Long size (decrement by 4)
    a[2] = 0x304;
    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressPreDecrement(2),
        Size::Long,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x300),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(a[2], 0x300);
    assert_eq!(cycles, 6);

    // Test Stack Pointer (A7) with Byte size (should decrement by 2)
    a[7] = 0x402;
    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressPreDecrement(7),
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x400),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(a[7], 0x400); // Special case for A7
    assert_eq!(cycles, 6);
}

#[test]
fn test_address_displacement() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0x100;
    let mut mem = Memory::new(1024);

    // Displacement word: 0x0010 (16)
    mem.write_word(0x100, 0x0010);
    a[0] = 0x200;

    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressDisplacement(0),
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x210),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x102); // PC advanced by 2
    assert_eq!(cycles, 8);

    // Negative displacement: 0xFFF0 (-16)
    pc = 0x102;
    mem.write_word(0x102, 0xFFF0);
    a[1] = 0x300;

    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressDisplacement(1),
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x2F0),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x104);
    assert_eq!(cycles, 8);
}

#[test]
fn test_address_index() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0x100;
    let mut mem = Memory::new(1024);

    // Extension word: D1.W, displacement 4
    // D/A = 0 (D), Reg = 1, W/L = 0 (W), Scale = 0 (always 0 on 68000), Disp = 4
    // 0001 0000 0000 0100 = 0x1004
    mem.write_word(0x100, 0x1004);
    a[0] = 0x200;
    d[1] = 0x20;

    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressIndex(0),
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    // EA = (An) + (Xn) + d8
    // EA = 0x200 + 0x20 + 4 = 0x224
    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x224),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x102);
    assert_eq!(cycles, 10);

    // Test with Address Register as Index
    // Extension word: A2.L, displacement -4
    // D/A = 1 (A), Reg = 2, W/L = 1 (L), Disp = -4 (0xFC)
    // 1010 1000 1111 1100 = 0xA8FC
    pc = 0x102;
    mem.write_word(0x102, 0xA8FC);
    a[0] = 0x300;
    a[2] = 0x100;

    let (ea, cycles) = calculate_ea(
        AddressingMode::AddressIndex(0),
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    // EA = 0x300 + 0x100 - 4 = 0x3FC
    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x3FC),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x104);
    assert_eq!(cycles, 10);
}

#[test]
fn test_absolute_short() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0x100;
    let mut mem = Memory::new(1024);

    // Absolute Short Address: 0x8000 (sign extended to 0xFFFF8000)
    mem.write_word(0x100, 0x8000);

    let (ea, cycles) = calculate_ea(
        AddressingMode::AbsoluteShort,
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0xFFFF8000),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x102);
    assert_eq!(cycles, 8);

    // Positive short address: 0x1234 -> 0x00001234
    pc = 0x102;
    mem.write_word(0x102, 0x1234);

    let (ea, cycles) = calculate_ea(
        AddressingMode::AbsoluteShort,
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x1234),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x104);
    assert_eq!(cycles, 8);
}

#[test]
fn test_absolute_long() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0x100;
    let mut mem = Memory::new(1024);

    // Absolute Long Address: 0x12345678
    mem.write_long(0x100, 0x12345678);

    let (ea, cycles) = calculate_ea(
        AddressingMode::AbsoluteLong,
        Size::Long,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x12345678),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x104);
    assert_eq!(cycles, 12);
}

#[test]
fn test_pc_displacement() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0x100;
    let mut mem = Memory::new(1024);

    // Displacement: 0x0020
    mem.write_word(0x100, 0x0020);

    let (ea, cycles) = calculate_ea(
        AddressingMode::PcDisplacement,
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    // EA = PC + d16
    // Note: PC is the address of the extension word (0x100)
    // EA = 0x100 + 0x20 = 0x120
    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x120),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x102);
    assert_eq!(cycles, 8);
}

#[test]
fn test_pc_index() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0x100;
    let mut mem = Memory::new(1024);

    // Extension word: D0.W, displacement 10
    // D/A = 0 (D), Reg = 0, W/L = 0 (W), Disp = 10
    // 0000 0000 0000 1010 = 0x000A
    mem.write_word(0x100, 0x000A);
    d[0] = 0x40;

    let (ea, cycles) = calculate_ea(
        AddressingMode::PcIndex,
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    // EA = PC + Xn + d8
    // PC = 0x100 (base of extension word)
    // EA = 0x100 + 0x40 + 10 = 0x14A
    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x14A),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x102);
    assert_eq!(cycles, 10);
}

#[test]
fn test_immediate() {
    let mut d = [0; 8];
    let mut a = [0; 8];
    let mut pc = 0x100;
    let mut mem = Memory::new(1024);

    // Immediate Word
    mem.write_word(0x100, 0x1234);

    let (ea, cycles) = calculate_ea(
        AddressingMode::Immediate,
        Size::Word,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    // Should point to the immediate data
    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x100),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x102);
    assert_eq!(cycles, 4); // 1 extension word * 4

    // Immediate Long
    pc = 0x102;
    mem.write_long(0x102, 0x12345678);

    let (ea, cycles) = calculate_ea(
        AddressingMode::Immediate,
        Size::Long,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x102),
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x106);
    assert_eq!(cycles, 8); // 2 extension words * 4

    // Immediate Byte (occupies a word, skipping a byte)
    // The implementation of calculate_ea for Immediate Byte adds 1 to the address
    // to point to the low byte of the word.
    pc = 0x106;
    mem.write_word(0x106, 0x00FF);

    let (ea, cycles) = calculate_ea(
        AddressingMode::Immediate,
        Size::Byte,
        &mut d,
        &mut a,
        &mut pc,
        &mut mem,
    );

    match ea {
        EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x107), // 0x106 + 1
        _ => panic!("Wrong EA type"),
    }
    assert_eq!(pc, 0x108); // Advances by 2 bytes (1 word)
    assert_eq!(cycles, 4);
}
