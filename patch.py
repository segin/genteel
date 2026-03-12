import re

with open("src/z80/tests_block.rs", "r") as f:
    content = f.read()

replacement = """
fn setup_ldir_params(rng: &mut Rng) -> (u16, u16, u16) {
    let src = rng.next_u16();
    let dst = rng.next_u16();
    // Weighted length distribution: heavy on small, some large, some 0
    let len_case = rng.next() % 10;
    let len = if len_case < 7 {
        (rng.next() % 32) as u16
    } else if len_case < 9 {
        (rng.next() % 1024) as u16
    } else {
        // 0 is handled as 65536, let's test specific BC=0 separately generally,
        // but include some random large ones
        rng.next_u16()
    };

    let bc = len as u16;
    (src, dst, bc)
}

fn init_ldir_memory<M: MemoryInterface, I: crate::memory::IoInterface>(
    cpu: &mut Z80<M, I>,
    src: u16,
    dst: u16,
    bc: u16,
    rng: &mut Rng,
) -> Vec<u8> {
    // Fill memory with random junk
    // We can't fill 64k every time (too slow).
    // Just fill the affected source range.
    let mut ref_mem = snapshot_memory(cpu);

    // Fill source area in both
    let real_len = if bc == 0 { 0x10000 } else { bc as usize };
    // Limit fill to avoid timeout on huge BC=0 tests in loop
    // If BC=0 (64k), we only fill a subset or accept 0s.
    // Let's rely on 'junk' fill:
    // Fill a window around src and dst
    for k in 0..256 {
        let val = rng.next_u8();
        let s_addr = src.wrapping_add(k) as usize;
        let d_addr = dst.wrapping_add(k) as usize;
        cpu.memory.write_byte(s_addr as u32, val);
        ref_mem[s_addr] = val;
        cpu.memory.write_byte(d_addr as u32, val ^ 0xFF); // Different initial dst
        ref_mem[d_addr] = val ^ 0xFF;
    }
    // Also fill end of range
    if real_len > 256 {
        let val = rng.next_u8();
        let s_addr = src.wrapping_add((real_len - 1) as u16) as usize;
        cpu.memory.write_byte(s_addr as u32, val);
        ref_mem[s_addr] = val;
    }
    ref_mem
}

fn execute_ldir_loop<M: MemoryInterface, I: crate::memory::IoInterface>(
    cpu: &mut Z80<M, I>,
    bc: u16,
) {
    let mut steps = 0;
    loop {
        // Check if we are about to execute LDIR
        // If PC == 0, we are at LDIR.
        // If instructions can be overwritten (self-modifying code), check that too?
        // If LDIR overwrites itself, behavior is undefined/complex.
        // We assume test cases generally don't overwrite 0x0000 unless random src/dst hits it.
        // If so, both ref and cpu behavior should arguably match or diverge.
        // For chaos test, let's accept divergence if code is overwritten.
        // But checking code integrity complicates things.
        // Let's check if code is intact.
        if cpu.memory.read_byte(0 as u32) != 0xED || cpu.memory.read_byte(1 as u32) != 0xB0 {
            // Code overwritten. Skip verification of this insane case.
            break;
        }

        cpu.step();
        steps += 1;
        if cpu.pc != 0 {
            break;
        } // Loop done
        if steps > 70000 {
            panic!("LDIR infinite loop or too long? BC={}", bc);
        }
    }
}

fn validate_ldir_result<M: MemoryInterface, I: crate::memory::IoInterface>(
    cpu: &Z80<M, I>,
    exp_hl: u16,
    exp_de: u16,
    exp_bc: u16,
    ref_mem: &[u8],
    bc: u16,
    i: usize,
    rng: &mut Rng,
) {
    if cpu.memory.read_byte(0 as u32) == 0xED {
        // valid result check
        assert_eq!(cpu.hl(), exp_hl, "HL mismatch case #{}", i);
        assert_eq!(cpu.de(), exp_de, "DE mismatch case #{}", i);
        assert_eq!(cpu.bc(), exp_bc, "BC mismatch case #{}", i);

        // Check memory window
        // We can't check all 64k. Check random samples + boundaries.
        for _k in 0..50 {
            let offset = rng.next() as usize % 0x10000;
            assert_eq!(
                cpu.memory.read_byte(offset as u32),
                ref_mem[offset],
                "Mem mismatch at {} case #{} BC={}",
                offset,
                i,
                bc
            );
        }
    }
}

fn run_ldir_test_case(i: usize, rng: &mut Rng) {
    let (src, dst, bc) = setup_ldir_params(rng);

    // Setup Z80
    // We put the LDIR instruction at some safe place, e.g., 0x0000,
    // assuming src/dst don't overwrite it immediately.
    // To be safe, we execute until PC indicates completion.

    let mut cpu = create_z80(&[]);
    // Put Opcode at 0x100 avoids conflict usually?
    // Let's randomize PC placement too? No, keep simple.
    let _code_base = 0x0000;
    cpu.memory.write_byte(0 as u32, 0xED);
    cpu.memory.write_byte(1 as u32, 0xB0); // LDIR
    cpu.pc = 0;

    cpu.set_hl(src);
    cpu.set_de(dst);
    cpu.set_bc(bc);

    let mut ref_mem = init_ldir_memory(&mut cpu, src, dst, bc, rng);

    // Run Reference
    // Be careful with large BC in reference loop - it's fast in native code
    let (exp_hl, exp_de, exp_bc) = reference_ldir(&mut ref_mem, src, dst, bc);

    // Run Emulator
    // Step until PC moves past instruction
    // Safety Break
    execute_ldir_loop(&mut cpu, bc);

    // Validation
    validate_ldir_result(&cpu, exp_hl, exp_de, exp_bc, &ref_mem, bc, i, rng);
}
"""

# Now we need to carefully replace the original run_ldir_test_case logic with our new string
original_pattern = re.compile(r"fn run_ldir_test_case\(i: usize, rng: &mut Rng\) \{.*?\n\}\n", re.DOTALL)
new_content = original_pattern.sub(replacement, content, count=1)

with open("src/z80/tests_block.rs", "w") as f:
    f.write(new_content)

print("Replaced run_ldir_test_case")
