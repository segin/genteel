#[cfg(test)]
mod tests {
    use crate::cpu::Cpu;
    use crate::memory::{Memory, MemoryInterface};

    /// Every opcode, valid or not, must execute without panicking (the core
    /// runs untrusted ROM code), including with registers near the top of
    /// the address space.
    #[test]
    fn every_opcode_executes_without_panic() {
        let mut memory = Memory::new(0x1_0000);
        let mut cpu = Cpu::new(&mut memory);
        for opcode in 0..=0xFFFFu32 {
            for addr in (0..0x400).step_by(4) {
                memory.write_long(addr, 0x2000);
            }
            memory.write_word(0x1000, opcode as u16);
            memory.write_long(0x1002, 0x0000_7FFE);
            memory.write_long(0x1006, 0x0000_2000);
            cpu.halted = false;
            cpu.stopped = false;
            cpu.pc = 0x1000;
            cpu.sr = 0x2700;
            cpu.a = [
                0x4000,
                0x0000_FFF0,
                0xFFFF_FFF0,
                0x8000,
                0x10,
                0x6000,
                0x7FFF_FFF0,
                0x8000,
            ];
            cpu.d = [1, 0xFFFF_FFFF, 0x8000, 0, 7, 0x10000, 0x40, 3];
            cpu.step_instruction(&mut WrappingMemory(&mut memory));
        }
    }

    /// Wraps a small RAM so that any 32-bit address is in range.
    #[derive(Debug)]
    struct WrappingMemory<'a>(&'a mut Memory);

    impl MemoryInterface for WrappingMemory<'_> {
        fn read_byte(&mut self, a: u32) -> u8 {
            self.0.read_byte(a & 0xFFFF)
        }
        fn write_byte(&mut self, a: u32, v: u8) {
            self.0.write_byte(a & 0xFFFF, v)
        }
        fn read_word(&mut self, a: u32) -> u16 {
            self.0.read_word(a & 0xFFFE)
        }
        fn write_word(&mut self, a: u32, v: u16) {
            self.0.write_word(a & 0xFFFE, v)
        }
        fn read_long(&mut self, a: u32) -> u32 {
            ((self.read_word(a) as u32) << 16) | self.read_word(a.wrapping_add(2)) as u32
        }
        fn write_long(&mut self, a: u32, v: u32) {
            self.write_word(a, (v >> 16) as u16);
            self.write_word(a.wrapping_add(2), v as u16);
        }
    }
}
