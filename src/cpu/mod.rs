use crate::memory::Memory;

#[derive(Debug)]
pub struct Cpu {
    // Registers
    pub d: [u32; 8], // Data registers
    pub a: [u32; 8], // Address registers
    pub pc: u32,     // Program counter
    pub sr: u16,     // Status register
}

impl Cpu {
    pub fn new(memory: &Memory) -> Self {
        let mut cpu = Self {
            d: [0; 8],
            a: [0; 8],
            pc: 0,
            sr: 0x2700, // Supervisor mode, interrupts disabled
        };

        // At startup, the supervisor stack pointer is read from address 0x00000000
        // and the program counter is read from 0x00000004.
        cpu.a[7] = memory.read_long(0x0);
        cpu.pc = memory.read_long(0x4);

        cpu
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

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

        let cpu = Cpu::new(&memory);

        // A7 is the supervisor stack pointer
        assert_eq!(cpu.a[7], 0x1234);
        assert_eq!(cpu.pc, 0x5678);
        assert_eq!(cpu.sr, 0x2700);
    }
}
