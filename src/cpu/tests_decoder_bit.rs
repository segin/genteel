#[cfg(test)]
mod tests {
    use crate::cpu::decoder::{decode, BitSource, Instruction, AddressingMode};

    #[test]
    fn test_decode_bit_dynamic() {
        // BTST D1, D0
        // Opcode: 0000 001 0 00 000 000 = 0x0200 -> Wait.
        // Bit 8 must be set for dynamic bit op.
        // 0000 001 1 00 000 000 = 0x0300
        // Reg (D1): 001 (bits 11-9)
        // Mode (D0): 000 (bits 5-3)
        // EaReg (D0): 000 (bits 2-0)
        // Op (BTST): 00 (bits 7-6)
        let instr = decode(0x0300);
        assert_eq!(
            instr,
            Instruction::Btst {
                bit: BitSource::Register(1),
                dst: AddressingMode::DataRegister(0),
            }
        );

        // BCHG D2, (A0)
        // Opcode: 0000 010 1 01 010 000 = 0x0550
        // Reg (D2): 010 (bits 11-9)
        // Mode ((A0)): 010 (bits 5-3)
        // EaReg (A0): 000 (bits 2-0)
        // Op (BCHG): 01 (bits 7-6)
        let instr = decode(0x0550);
        assert_eq!(
            instr,
            Instruction::Bchg {
                bit: BitSource::Register(2),
                dst: AddressingMode::AddressIndirect(0),
            }
        );

        // BCLR D3, (A1)+
        // Opcode: 0000 011 1 10 011 001 = 0x0799
        // Reg (D3): 011 (bits 11-9)
        // Mode ((A1)+): 011 (bits 5-3)
        // EaReg (A1): 001 (bits 2-0)
        // Op (BCLR): 10 (bits 7-6)
        let instr = decode(0x0799);
        assert_eq!(
            instr,
            Instruction::Bclr {
                bit: BitSource::Register(3),
                dst: AddressingMode::AddressPostIncrement(1),
            }
        );

        // BSET D4, -(A2)
        // Opcode: 0000 100 1 11 100 010 = 0x09E2
        // Reg (D4): 100 (bits 11-9)
        // Mode (-(A2)): 100 (bits 5-3)
        // EaReg (A2): 010 (bits 2-0)
        // Op (BSET): 11 (bits 7-6)
        let instr = decode(0x09E2);
        assert_eq!(
            instr,
            Instruction::Bset {
                bit: BitSource::Register(4),
                dst: AddressingMode::AddressPreDecrement(2),
            }
        );
    }
}
