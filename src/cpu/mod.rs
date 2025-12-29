use crate::memory::Memory;

#[derive(Debug)]
pub struct Cpu {
    // Registers
    pub d: [u32; 8], // Data registers
    pub a: [u32; 8], // Address registers
    pub pc: u32,     // Program counter
    pub sr: u16,     // Status register
    memory: Memory,  // CPU owns the memory
}

impl Cpu {
    pub fn new(memory: Memory) -> Self {
        let mut cpu = Self {
            d: [0; 8],
            a: [0; 8],
            pc: 0,
            sr: 0x2700, // Supervisor mode, interrupts disabled
            memory, // Initialize with the passed memory
        };

        // At startup, the supervisor stack pointer is read from address 0x00000000
        // and the program counter is read from 0x00000004.
        cpu.a[7] = cpu.memory.read_long(0x0);
        cpu.pc = cpu.memory.read_long(0x4);

        cpu
    }

    pub fn step_instruction(&mut self) {
        let opcode = self.memory.read_word(self.pc);
        self.pc += 2; // Advance PC past the opcode

        // For now, only implement MOVE.L D1, D0 (0x2008), MOVE.B (0x1008), MOVE.W (0x3008)
        if opcode == 0x2008 { // MOVE.L D1, D0
            self.d[0] = self.d[1];
        } else if opcode == 0x1008 { // MOVE.B D1, D0
            self.d[0] = (self.d[0] & 0xFFFFFF00) | (self.d[1] & 0xFF);
        } else if opcode == 0x3008 { // MOVE.W D1, D0
            self.d[0] = (self.d[0] & 0xFFFF0000) | (self.d[1] & 0xFFFF);
        } else {
            // Handle unimplemented opcodes or NOP for now
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;
    use proptest::prelude::*;

    #[test]
    fn initial_state_with_memory() {
        let mut memory = Memory::new(1024); // 1KB of memory for the test
        // Initial SP from 0x00000000
        memory.data[0] = 0x00;
        memory.data[1] = 0x00;
        memory.data[2] = 0x12;
        memory.data[3] = 0x34;
        // Initial PC from 0x00000004
        memory.data[4] = 0x00;
        memory.data[5] = 0x00;
        memory.data[6] = 0x56;
        memory.data[7] = 0x78;

        let cpu = Cpu::new(memory); // Cpu now owns memory

        // A7 is the supervisor stack pointer
        assert_eq!(cpu.a[7], 0x1234);
        assert_eq!(cpu.pc, 0x5678);
        assert_eq!(cpu.sr, 0x2700);
    }

    #[test]
    fn test_move_l_d1_d0() {
        let mut memory = Memory::new(1024);
        memory.data[8] = 0x20;
        memory.data[9] = 0x08;

        memory.data[4] = 0x00; memory.data[5] = 0x00; memory.data[6] = 0x00; memory.data[7] = 0x08;

        let mut cpu = Cpu::new(memory);
        cpu.d[1] = 0xABCD1234;

        cpu.step_instruction();

        assert_eq!(cpu.d[0], 0xABCD1234);
        assert_eq!(cpu.pc, 0x0000000A);
    }

    #[test]
    fn test_move_b_d1_d0() {
        let mut memory = Memory::new(1024);
        memory.data[8] = 0x10;
        memory.data[9] = 0x08;

        memory.data[4] = 0x00; memory.data[5] = 0x00; memory.data[6] = 0x00; memory.data[7] = 0x08;

        let mut cpu = Cpu::new(memory);
        cpu.d[1] = 0xABCD1234;
        cpu.d[0] = 0xEEEEEEEE;

        cpu.step_instruction();

        assert_eq!(cpu.d[0], 0xEEEEEE34);
        assert_eq!(cpu.pc, 0x0000000A);
    }

    #[test]
    fn test_move_w_d1_d0() {
        let mut memory = Memory::new(1024);
        memory.data[8] = 0x30;
        memory.data[9] = 0x08;

        memory.data[4] = 0x00; memory.data[5] = 0x00; memory.data[6] = 0x00; memory.data[7] = 0x08;

        let mut cpu = Cpu::new(memory);
        cpu.d[1] = 0xABCD1234;
        cpu.d[0] = 0xEEEEEEEE;

        cpu.step_instruction();

        assert_eq!(cpu.d[0], 0xEEEE1234);
        assert_eq!(cpu.pc, 0x0000000A);
    }

    proptest! {
        #[test]
        fn test_move_l_d1_d0_proptest(val in 0..u32::MAX) {
            let mut memory = Memory::new(1024);
            memory.data[8] = 0x20;
            memory.data[9] = 0x08;

            memory.data[4] = 0x00; memory.data[5] = 0x00; memory.data[6] = 0x00; memory.data[7] = 0x08;

            let mut cpu = Cpu::new(memory);
            cpu.d[1] = val;

            cpu.step_instruction();

            assert_eq!(cpu.d[0], val);
        }

        #[test]
        fn test_move_b_d1_d0_proptest(val in 0..=0xFFu32) {
            let mut memory = Memory::new(1024);
            memory.data[8] = 0x10;
            memory.data[9] = 0x08;

            memory.data[4] = 0x00; memory.data[5] = 0x00; memory.data[6] = 0x00; memory.data[7] = 0x08;

            let mut cpu = Cpu::new(memory);
            cpu.d[1] = val;
            cpu.d[0] = 0xEEEEEEEE;

            cpu.step_instruction();

            assert_eq!(cpu.d[0], 0xEEEEEE00 | val);
        }

        #[test]
        fn test_move_w_d1_d0_proptest(val in 0..=0xFFFFu32) {
            let mut memory = Memory::new(1024);
            memory.data[8] = 0x30;
            memory.data[9] = 0x08;

            memory.data[4] = 0x00; memory.data[5] = 0x00; memory.data[6] = 0x00; memory.data[7] = 0x08;

            let mut cpu = Cpu::new(memory);
            cpu.d[1] = val;
            cpu.d[0] = 0xEEEEEEEE;

            cpu.step_instruction();

            assert_eq!(cpu.d[0], 0xEEEE0000 | val);
        }
    }
}
