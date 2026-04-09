use crate::cpu::{flags, Cpu};
use crate::memory::{Memory, MemoryInterface};

pub fn create_test_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x10000);
    memory.write_long(0, 0x1000); // SP
    memory.write_long(4, 0x100); // PC
    let cpu = Cpu::new(&mut memory);
    (cpu, memory)
}

pub fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x100000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    cpu.sr = flags::SUPERVISOR; // Supervisor, Mask 0
    (cpu, memory)
}

pub fn write_op(memory: &mut Memory, opcodes: &[u16]) {
    let mut addr = 0x1000u32;
    for &op in opcodes {
        memory.write_word(addr, op);
        addr += 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_cpu() {
        let (cpu, mut memory) = create_test_cpu();
        assert_eq!(memory.read_long(0), 0x1000); // SP
        assert_eq!(memory.read_long(4), 0x100); // PC
        assert_eq!(cpu.a[7], 0x1000); // PC will be initialized from address 4, SP from 0
        assert_eq!(cpu.pc, 0x100);
    }

    #[test]
    fn test_create_cpu() {
        let (cpu, _memory) = create_cpu();
        assert_eq!(cpu.pc, 0x1000);
        assert_eq!(cpu.a[7], 0x8000);
        assert_eq!(cpu.sr, flags::SUPERVISOR);
    }

    #[test]
    fn test_write_op() {
        let mut memory = Memory::new(0x2000);
        let opcodes = [0x1234, 0x5678, 0x9ABC];
        write_op(&mut memory, &opcodes);
        assert_eq!(memory.read_word(0x1000), 0x1234);
        assert_eq!(memory.read_word(0x1002), 0x5678);
        assert_eq!(memory.read_word(0x1004), 0x9ABC);
    }
}
