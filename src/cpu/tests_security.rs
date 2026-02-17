#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::memory::Memory;

    fn create_test_cpu() -> (Cpu, Memory) {
        let mut memory = Memory::new(0x10000);
        // Initial SP and PC
        memory.write_long(0, 0x1000); // SP
        memory.write_long(4, 0x100); // PC
        let cpu = Cpu::new(&mut memory);
        (cpu, memory)
    }

    #[test]
    fn test_cache_out_of_bounds() {
        let (mut cpu, mut memory) = create_test_cpu();

        // NOP instruction
        memory.write_word(0x100, 0x4E71);

        // Replace cache with an empty slice
        cpu.decode_cache = vec![].into_boxed_slice();

        // This should not panic or crash
        let cycles = cpu.step_instruction(&mut memory);

        assert_eq!(cycles, 4); // NOP takes 4 cycles
        assert_eq!(cpu.pc, 0x102);
    }
}
