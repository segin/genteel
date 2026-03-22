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
