import os
import glob

files = glob.glob('src/z80/tests*.rs')

for filepath in files:
    if filepath == 'src/z80/tests.rs': continue # Already done
    if filepath == 'src/z80/tests_alu.rs': continue # Already done

    with open(filepath, 'r') as f:
        content = f.read()

    # Replace return type
    old_sig = 'fn z80(program: &[u8]) -> Z80<crate::memory::Memory, crate::z80::test_utils::TestIo> {'
    new_sig = 'fn z80(program: &[u8]) -> crate::z80::test_utils::TestZ80 {'

    if old_sig in content:
        content = content.replace(old_sig, new_sig)

        # Replace instantiation
        # Pattern: Z80::new(m, crate::z80::test_utils::TestIo::default())
        old_init = 'Z80::new(m, crate::z80::test_utils::TestIo::default())'
        new_init = 'let cpu = Z80::new();\n    crate::z80::test_utils::TestZ80::new(cpu, m, crate::z80::test_utils::TestIo::default())'

        content = content.replace(old_init, new_init)

        print(f"Updated {filepath}")

        with open(filepath, 'w') as f:
            f.write(content)
    else:
        print(f"Signature not found in {filepath}")

    # Handle snapshot_memory if present (tests_block.rs)
    if 'snapshot_memory' in content:
        old_snap = 'fn snapshot_memory<M: MemoryInterface, I: crate::memory::IoInterface>(\n    z80: &mut Z80<M, I>,\n) -> Vec<u8> {'
        new_snap = 'fn snapshot_memory(z80: &mut crate::z80::test_utils::TestZ80) -> Vec<u8> {'
        if old_snap in content:
             content = content.replace(old_snap, new_snap)
             print(f"Updated snapshot_memory in {filepath}")
             with open(filepath, 'w') as f:
                f.write(content)
        else:
             # Try single line version if any
             old_snap_sl = 'fn snapshot_memory<M: MemoryInterface, I: crate::memory::IoInterface>(z80: &mut Z80<M, I>) -> Vec<u8> {'
             if old_snap_sl in content:
                 content = content.replace(old_snap_sl, new_snap)
                 print(f"Updated snapshot_memory (SL) in {filepath}")
                 with open(filepath, 'w') as f:
                    f.write(content)
