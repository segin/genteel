use crate::cpu::decoder::{decode, AddressingMode, Instruction, ShiftCount, Size};

// Helper macro for testing
macro_rules! check_decode {
    ($opcode:expr, $expected:expr) => {
        let instr = decode($opcode);
        assert_eq!(instr, $expected, "Opcode 0x{:04X}", $opcode);
    };
}

#[test]
fn test_decode_asl_asr_register_immediate() {
    // ASL.B #1, D0
    // Opcode: 1110 001 1 00 0 00 000 = 0xE300
    check_decode!(
        0xE300,
        Instruction::Asl {
            size: Size::Byte,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Immediate(1),
        }
    );

    // ASL.W #8, D0 (count 0 encodes 8)
    // Opcode: 1110 000 1 01 0 00 000 = 0xE140
    check_decode!(
        0xE140,
        Instruction::Asl {
            size: Size::Word,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Immediate(8),
        }
    );

    // ASR.L #4, D7
    // Opcode: 1110 100 0 10 0 00 111 = 0xE887
    check_decode!(
        0xE887,
        Instruction::Asr {
            size: Size::Long,
            dst: AddressingMode::DataRegister(7),
            count: ShiftCount::Immediate(4),
        }
    );
}

#[test]
fn test_decode_asl_asr_register_count() {
    // ASL.B D1, D0
    // Opcode: 1110 001 1 00 1 00 000 = 0xE320
    check_decode!(
        0xE320,
        Instruction::Asl {
            size: Size::Byte,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Register(1),
        }
    );

    // ASR.W D2, D3
    // Opcode: 1110 010 0 01 1 00 011 = 0xE463
    check_decode!(
        0xE463,
        Instruction::Asr {
            size: Size::Word,
            dst: AddressingMode::DataRegister(3),
            count: ShiftCount::Register(2),
        }
    );
}

#[test]
fn test_decode_asl_asr_memory() {
    // ASL.W (A0)
    // Opcode: 1110 000 1 11 010 000 = 0xE1D0
    check_decode!(
        0xE1D0,
        Instruction::Asl {
            size: Size::Word,
            dst: AddressingMode::AddressIndirect(0),
            count: ShiftCount::Immediate(1),
        }
    );

    // ASR.W (A0)+
    // Opcode: 1110 000 0 11 011 000 = 0xE0D8
    check_decode!(
        0xE0D8,
        Instruction::Asr {
            size: Size::Word,
            dst: AddressingMode::AddressPostIncrement(0),
            count: ShiftCount::Immediate(1),
        }
    );
}

#[test]
fn test_decode_lsl_lsr() {
    // LSL.B #1, D0
    // Opcode: 1110 001 1 00 0 01 000 = 0xE308
    check_decode!(
        0xE308,
        Instruction::Lsl {
            size: Size::Byte,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Immediate(1),
        }
    );

    // LSR.W D1, D0
    // Opcode: 1110 001 0 01 1 01 000 = 0xE268
    check_decode!(
        0xE268,
        Instruction::Lsr {
            size: Size::Word,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Register(1),
        }
    );

    // LSL.W (A0)
    // Opcode: 1110 001 1 11 010 000 = 0xE3D0
    check_decode!(
        0xE3D0,
        Instruction::Lsl {
            size: Size::Word,
            dst: AddressingMode::AddressIndirect(0),
            count: ShiftCount::Immediate(1),
        }
    );
}

#[test]
fn test_decode_rol_ror() {
    // ROL.B #1, D0
    // Opcode: 1110 001 1 00 0 11 000 = 0xE318
    check_decode!(
        0xE318,
        Instruction::Rol {
            size: Size::Byte,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Immediate(1),
        }
    );

    // ROR.L D1, D0
    // Opcode: 1110 001 0 10 1 11 000 = 0xE2B8
    check_decode!(
        0xE2B8,
        Instruction::Ror {
            size: Size::Long,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Register(1),
        }
    );

    // ROL.W -(A0)
    // Opcode: 1110 011 1 11 100 000 = 0xE7E0
    check_decode!(
        0xE7E0,
        Instruction::Rol {
            size: Size::Word,
            dst: AddressingMode::AddressPreDecrement(0),
            count: ShiftCount::Immediate(1),
        }
    );
}

#[test]
fn test_decode_roxl_roxr() {
    // ROXL.B #1, D0
    // Opcode: 1110 001 1 00 0 10 000 = 0xE310
    check_decode!(
        0xE310,
        Instruction::Roxl {
            size: Size::Byte,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Immediate(1),
        }
    );

    // ROXR.W #8, D0
    // Opcode: 1110 000 0 01 0 10 000 = 0xE050
    check_decode!(
        0xE050,
        Instruction::Roxr {
            size: Size::Word,
            dst: AddressingMode::DataRegister(0),
            count: ShiftCount::Immediate(8),
        }
    );

    // ROXL.W (xxx).W
    // Opcode: 1110 010 1 11 111 000 = 0xE5F8 (Mode 7, Reg 0)
    check_decode!(
        0xE5F8,
        Instruction::Roxl {
            size: Size::Word,
            dst: AddressingMode::AbsoluteShort,
            count: ShiftCount::Immediate(1),
        }
    );
}
