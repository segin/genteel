#[cfg(test)]
mod tests {
    use crate::cpu::decoder::DecodeCacheEntry;
    use crate::cpu::Cpu;
    use crate::memory::{Memory, MemoryInterface};

    #[test]
    fn test_unsafe_cache_access() {
        let mut memory = Memory::new(0x10000); // 64KB memory
        let mut cpu = Cpu::new(&mut memory);

        // Exploit: Replace cache with a tiny one
        cpu.decode_cache = vec![DecodeCacheEntry::default(); 1].into();

        // PC at 0x100 maps to index 0x80 (128)
        cpu.pc = 0x100;

        // Ensure memory at PC has a valid instruction (NOP)
        // NOP = 0x4E71
        memory.write_word(0x100, 0x4E71);

        // With the fix, this should handle the out-of-bounds cache index gracefully
        // by falling back to uncached fetch, executing the instruction, and continuing.
        let cycles = cpu.step_instruction(&mut memory);

        // Check execution results
        assert_eq!(cycles, 4, "NOP should take 4 cycles");
        assert_eq!(cpu.pc, 0x102, "PC should advance by 2");
    }
}
