use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

pub fn create_test_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x10000);
    memory.write_long(0, 0x1000); // SP
    memory.write_long(4, 0x100); // PC
    let cpu = Cpu::new(&mut memory);
    (cpu, memory)
}
