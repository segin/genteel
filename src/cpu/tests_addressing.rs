#[cfg(test)]
mod tests {
    use crate::cpu::addressing::{calculate_ea, EffectiveAddress};
    use crate::cpu::decoder::{AddressingMode, Size};
    use crate::memory::{Memory, MemoryInterface};

    fn setup_cpu_state() -> ([u32; 8], [u32; 8], u32, Memory) {
        let d = [0; 8];
        let a = [0; 8];
        let pc = 0x100;
        let memory = Memory::new(0x10000); // 64KB memory
        (d, a, pc, memory)
    }

    #[test]
    fn test_ea_data_register() {
        let (mut d, mut a, mut pc, mut memory) = setup_cpu_state();

        // Test DataRegister mode for all registers
        for reg in 0..8 {
            let mode = AddressingMode::DataRegister(reg);
            let (ea, cycles) = calculate_ea(mode, Size::Long, &mut d, &mut a, &mut pc, &mut memory);

            assert_eq!(cycles, 0, "DataRegister mode should take 0 cycles");
            match ea {
                EffectiveAddress::DataRegister(r) => assert_eq!(r, reg),
                _ => panic!("Expected DataRegister EA"),
            }
        }
    }

    #[test]
    fn test_ea_address_register() {
        let (mut d, mut a, mut pc, mut memory) = setup_cpu_state();

        // Test AddressRegister mode for all registers
        for reg in 0..8 {
            let mode = AddressingMode::AddressRegister(reg);
            let (ea, cycles) = calculate_ea(mode, Size::Long, &mut d, &mut a, &mut pc, &mut memory);

            assert_eq!(cycles, 0, "AddressRegister mode should take 0 cycles");
            match ea {
                EffectiveAddress::AddressRegister(r) => assert_eq!(r, reg),
                _ => panic!("Expected AddressRegister EA"),
            }
        }
    }

    #[test]
    fn test_ea_address_indirect() {
        let (mut d, mut a, mut pc, mut memory) = setup_cpu_state();
        a[2] = 0x2000;

        let mode = AddressingMode::AddressIndirect(2);
        let (ea, cycles) = calculate_ea(mode, Size::Long, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 4);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x2000),
            _ => panic!("Expected Memory EA"),
        }
        // Address register should not change
        assert_eq!(a[2], 0x2000);
    }

    #[test]
    fn test_ea_post_increment() {
        let (mut d, mut a, mut pc, mut memory) = setup_cpu_state();

        // Test Size::Byte (increment by 1, except A7)
        a[0] = 0x1000;
        let mode = AddressingMode::AddressPostIncrement(0);
        let (ea, cycles) = calculate_ea(mode, Size::Byte, &mut d, &mut a, &mut pc, &mut memory);
        assert_eq!(cycles, 4);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x1000),
            _ => panic!("Expected Memory EA"),
        }
        assert_eq!(a[0], 0x1001);

        // Test Size::Word (increment by 2)
        a[1] = 0x2000;
        let mode = AddressingMode::AddressPostIncrement(1);
        let (ea, _) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x2000);
        }
        assert_eq!(a[1], 0x2002);

        // Test Size::Long (increment by 4)
        a[2] = 0x3000;
        let mode = AddressingMode::AddressPostIncrement(2);
        let (ea, _) = calculate_ea(mode, Size::Long, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x3000);
        }
        assert_eq!(a[2], 0x3004);

        // Test A7 (SP) special case for Byte (increment by 2)
        a[7] = 0x4000;
        let mode = AddressingMode::AddressPostIncrement(7);
        let (ea, _) = calculate_ea(mode, Size::Byte, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x4000);
        }
        assert_eq!(a[7], 0x4002); // Should be +2, not +1
    }

    #[test]
    fn test_ea_pre_decrement() {
        let (mut d, mut a, mut pc, mut memory) = setup_cpu_state();

        // Test Size::Byte (decrement by 1, except A7)
        a[0] = 0x1001;
        let mode = AddressingMode::AddressPreDecrement(0);
        let (ea, cycles) = calculate_ea(mode, Size::Byte, &mut d, &mut a, &mut pc, &mut memory);
        assert_eq!(cycles, 6);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x1000),
            _ => panic!("Expected Memory EA"),
        }
        assert_eq!(a[0], 0x1000);

        // Test Size::Word (decrement by 2)
        a[1] = 0x2002;
        let mode = AddressingMode::AddressPreDecrement(1);
        let (ea, _) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x2000);
        }
        assert_eq!(a[1], 0x2000);

        // Test Size::Long (decrement by 4)
        a[2] = 0x3004;
        let mode = AddressingMode::AddressPreDecrement(2);
        let (ea, _) = calculate_ea(mode, Size::Long, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x3000);
        }
        assert_eq!(a[2], 0x3000);

        // Test A7 (SP) special case for Byte (decrement by 2)
        a[7] = 0x4002;
        let mode = AddressingMode::AddressPreDecrement(7);
        let (ea, _) = calculate_ea(mode, Size::Byte, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x4000);
        }
        assert_eq!(a[7], 0x4000); // Should be -2, not -1
    }

    #[test]
    fn test_ea_displacement() {
        let (mut d, mut a, _, mut memory) = setup_cpu_state();
        let mut pc = 0x200;
        a[0] = 0x1000;

        // Write displacement word at PC
        memory.write_word(0x200, 0x0010); // +16

        let mode = AddressingMode::AddressDisplacement(0);
        let (ea, cycles) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 8);
        assert_eq!(pc, 0x202); // PC should advance by 2
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x1010), // 0x1000 + 0x10
            _ => panic!("Expected Memory EA"),
        }

        // Test negative displacement
        a[1] = 0x2000;
        pc = 0x300;
        memory.write_word(0x300, 0xFFF0); // -16

        let mode = AddressingMode::AddressDisplacement(1);
        let (ea, _) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x1FF0); // 0x2000 - 16
        }
    }

    #[test]
    fn test_ea_index() {
        let (mut d, mut a, _, mut memory) = setup_cpu_state();
        let mut pc = 0x200;
        a[0] = 0x1000;
        d[1] = 0x0000_0010; // Index +16

        // Extension word: D1.L, Scale=1 (implied), Displacement=4
        // D/A=0 (Dn), Reg=1, W/L=1 (Long), Scale=0 (reserved/1), Disp=4
        // 0 001 1 000 00000100 = 0x1804
        memory.write_word(0x200, 0x1804);

        let mode = AddressingMode::AddressIndex(0);
        let (ea, cycles) = calculate_ea(mode, Size::Byte, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 10);
        assert_eq!(pc, 0x202);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x1014), // 0x1000 + 0x10 + 4
            _ => panic!("Expected Memory EA"),
        }

        // Test Word index (signed)
        d[2] = 0xFFFF_FFFE; // -2 in lower word, but upper is junk if treated as word
        a[3] = 0x2000;
        pc = 0x300;

        // Extension word: D2.W, Disp=-2
        // D/A=0 (Dn), Reg=2, W/L=0 (Word), Scale=0, Disp=0xFE (-2)
        // 0 010 0 000 11111110 = 0x20FE
        memory.write_word(0x300, 0x20FE);

        let mode = AddressingMode::AddressIndex(3); // (A3)
        let (ea, _) = calculate_ea(mode, Size::Byte, &mut d, &mut a, &mut pc, &mut memory);

        // Base(0x2000) + Index(-2) + Disp(-2) = 0x1FFC
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x1FFC);
        }
    }

    #[test]
    fn test_ea_absolute_short() {
        let (mut d, mut a, _, mut memory) = setup_cpu_state();
        let mut pc = 0x200;

        // Absolute Short address: 0x8000 (sign extended to 0xFFFF8000)
        memory.write_word(0x200, 0x8000);

        let mode = AddressingMode::AbsoluteShort;
        let (ea, cycles) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 8);
        assert_eq!(pc, 0x202);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0xFFFF8000),
            _ => panic!("Expected Memory EA"),
        }

        // Positive short address: 0x1000
        pc = 0x300;
        memory.write_word(0x300, 0x1000);
        let (ea, _) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x00001000);
        }
    }

    #[test]
    fn test_ea_absolute_long() {
        let (mut d, mut a, _, mut memory) = setup_cpu_state();
        let mut pc = 0x200;

        // Absolute Long address: 0x00FF0000
        memory.write_long(0x200, 0x00FF0000);

        let mode = AddressingMode::AbsoluteLong;
        let (ea, cycles) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 12);
        assert_eq!(pc, 0x204);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x00FF0000),
            _ => panic!("Expected Memory EA"),
        }
    }

    #[test]
    fn test_ea_pc_displacement() {
        let (mut d, mut a, _, mut memory) = setup_cpu_state();
        let mut pc = 0x200;

        // PC Displacement: d16(PC)
        // 0x200: 0x0010 (+16)
        memory.write_word(0x200, 0x0010);

        let mode = AddressingMode::PcDisplacement;
        let (ea, cycles) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 8);
        assert_eq!(pc, 0x202);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x210), // 0x200 + 0x10
            _ => panic!("Expected Memory EA"),
        }

        // Negative displacement
        pc = 0x300;
        // 0x300: 0xFFF0 (-16)
        memory.write_word(0x300, 0xFFF0);
        let (ea, _) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x2F0); // 0x300 - 16
        }
    }

    #[test]
    fn test_ea_pc_index() {
        let (mut d, mut a, _, mut memory) = setup_cpu_state();
        let mut pc = 0x200;
        d[0] = 0x00000010; // Index +16

        // PC Index: d8(PC, Xn)
        // Extension word: D0.L, Disp=4
        // 0 000 1 000 00000100 = 0x0804
        memory.write_word(0x200, 0x0804);

        let mode = AddressingMode::PcIndex;
        let (ea, cycles) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 10);
        assert_eq!(pc, 0x202);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x214), // 0x200 + 16 + 4
            _ => panic!("Expected Memory EA"),
        }
    }

    #[test]
    fn test_ea_immediate() {
        let (mut d, mut a, _, mut memory) = setup_cpu_state();
        let mut pc = 0x200;

        // Immediate Word: 0x1234
        memory.write_word(0x200, 0x1234);

        let mode = AddressingMode::Immediate;
        let (ea, cycles) = calculate_ea(mode, Size::Word, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 4);
        assert_eq!(pc, 0x202);
        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x200),
            _ => panic!("Expected Memory EA"),
        }

        // Immediate Long: 0x12345678
        pc = 0x300;
        memory.write_long(0x300, 0x12345678);
        let (ea, cycles) = calculate_ea(mode, Size::Long, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 8); // 4 * 2 words
        assert_eq!(pc, 0x304);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x300);
        }

        // Immediate Byte: 0x12
        // Encoded as 00 12 (word), but EA points to low byte
        pc = 0x400;
        memory.write_word(0x400, 0x0012);
        let (ea, cycles) = calculate_ea(mode, Size::Byte, &mut d, &mut a, &mut pc, &mut memory);

        assert_eq!(cycles, 4);
        assert_eq!(pc, 0x402);
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x401); // Points to 0x12
        }
    }
}
