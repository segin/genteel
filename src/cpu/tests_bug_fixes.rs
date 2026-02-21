#[cfg(test)]
mod tests {
    use crate::cpu::decoder::{
        decode, AddressingMode, BitsInstruction, Instruction, ShiftCount, Size,
    };
    use crate::cpu::Cpu;
    use crate::memory::{Memory, MemoryInterface};

    fn create_test_cpu() -> (Cpu, Memory) {
        let mut memory = Memory::new(0x10000);
        // Initial SP and PC
        memory.write_long(0, 0x1000); // SP
        memory.write_long(4, 0x100); // PC
        let cpu = Cpu::new(&mut memory);
        (cpu, memory)
    }

    #[test]
    fn test_clr_predecrement_double_update_bug() {
        let (mut cpu, mut memory) = create_test_cpu();
        // CLR.B -(A0)
        // Opcode: 0100 001 0 00 100 000 = 0x4220
        memory.write_word(0x100, 0x4220);

        cpu.a[0] = 0x2000;

        // Expected behavior: A0 decremented by 1 (Byte) -> 0x1FFF. Memory at 0x1FFF cleared.
        cpu.step_instruction(&mut memory);

        // If bug exists (double decrement), A0 will be 0x1FFE.
        assert_eq!(cpu.a[0], 0x1FFF, "A0 should be decremented once by 1");
        assert_eq!(memory.read_byte(0x1FFF), 0);
    }

    #[test]
    fn test_clr_postincrement_double_update_bug() {
        let (mut cpu, mut memory) = create_test_cpu();
        // CLR.B (A0)+
        // Opcode: 0100 001 0 00 011 000 = 0x4218
        memory.write_word(0x100, 0x4218);

        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0xFF);

        // Expected behavior: A0 incremented by 1 -> 0x2001. Memory at 0x2000 cleared.
        cpu.step_instruction(&mut memory);

        // If bug exists (double increment), A0 will be 0x2002.
        assert_eq!(cpu.a[0], 0x2001, "A0 should be incremented once by 1");
        assert_eq!(memory.read_byte(0x2000), 0);
    }

    #[test]
    fn test_move_immediate_byte_read_bug() {
        let (mut cpu, mut memory) = create_test_cpu();
        // MOVE.B #$12, D0
        // Opcode: 0001 000 0 00 111 100 (MOVE.B #<data>, D0)
        // 103C 0012
        memory.write_word(0x100, 0x103C);
        memory.write_word(0x102, 0x0012);

        cpu.d[0] = 0;

        cpu.step_instruction(&mut memory);

        // If bug exists (reading high byte at PC), it reads 0x00 instead of 0x12.
        assert_eq!(cpu.d[0], 0x12, "Should move immediate byte 0x12");
    }

    #[test]
    fn test_addq_byte_to_an_bug() {
        let (mut cpu, mut memory) = create_test_cpu();
        // ADDQ.B #1, A0
        // Opcode: 0101 001 0 00 001 000 = 0x5208
        // Data=1 (001), Size=00 (Byte), Mode=001 (An), Reg=0
        memory.write_word(0x100, 0x5208);

        cpu.a[0] = 0x10000000;

        // Expected behavior: Illegal instruction.
        // It should take some cycles (34 for exception).
        // PC should jump to vector 4 (address 0x10).

        // Setup vector 4
        memory.write_long(0x10, 0x00004000);

        let _cycles = cpu.step_instruction(&mut memory);

        // If it executes as ADDQ: PC will be 0x102.
        // If it raises exception: PC will be 0x4000.

        if cpu.pc != 0x4000 {
            panic!(
                "ADDQ.B to An did not raise Illegal Instruction exception! PC={:X}",
                cpu.pc
            );
        }
    }

    #[test]
    fn test_decode_memory_shift_bug() {
        // ASL.W (A0)
        // Opcode: 1110 000 1 11 010 000 = E1D0
        let instr_asl = decode(0xE1D0);
        match instr_asl {
            Instruction::Bits(BitsInstruction::AslM { dst }) => {
                assert_eq!(dst, AddressingMode::AddressIndirect(0));
            }
            _ => panic!("Expected ASL, got {:?}", instr_asl),
        }

        // LSR.W (A0)
        // Opcode: 1110 001 0 11 010 000 = E2D0
        let instr_lsr = decode(0xE2D0);
        match instr_lsr {
            Instruction::Bits(BitsInstruction::Lsr { size, dst, count }) => {
                assert_eq!(size, Size::Word);
                assert_eq!(dst, AddressingMode::AddressIndirect(0));
                assert_eq!(count, ShiftCount::Immediate(1));
            }
            _ => panic!("Expected LSR, got {:?}", instr_lsr),
        }
    }
}
