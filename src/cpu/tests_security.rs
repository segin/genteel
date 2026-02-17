use super::Cpu;
use crate::memory::Memory;
use crate::memory::MemoryInterface;
use super::decoder::DecodeCacheEntry;

#[test]
fn test_cache_out_of_bounds() {
    let mut memory = Memory::new(0x10000);
    // Write NOP at 0x100
    memory.write_word(0x100, 0x4E71);

    // Initial state
    memory.write_long(0, 0x1000); // SP
    memory.write_long(4, 0x100);  // PC

    let mut cpu = Cpu::new(&mut memory);

    // Vulnerability reproduction: shrink the cache
    // The cache index for PC=0x100 is (0x100 >> 1) & 0xFFFF = 0x80 = 128.
    // If we make the cache smaller than 129, it should fail.
    // Let's make it empty.
    cpu.decode_cache = vec![DecodeCacheEntry::default(); 0].into_boxed_slice();

    // Execute instruction
    // This should panic or crash if the vulnerability exists (unsafe get_unchecked)
    // or return garbage/UB.
    cpu.step_instruction(&mut memory);

    // If we reach here, check that PC advanced (NOP takes 2 bytes)
    assert_eq!(cpu.pc, 0x102);
}
