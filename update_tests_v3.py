import glob

files = glob.glob('src/z80/tests*.rs')

patterns = [
    (
        'fn z80(program: &[u8]) -> Z80<crate::memory::Memory, crate::z80::test_utils::TestIo> {',
        'fn z80(program: &[u8]) -> crate::z80::test_utils::TestZ80 {'
    ),
    (
        'fn create_z80(program: &[u8]) -> Z80<crate::memory::Memory, crate::z80::test_utils::TestIo> {',
        'fn create_z80(program: &[u8]) -> crate::z80::test_utils::TestZ80 {'
    ),
    (
        'fn create_z80(\n    program: &[u8],\n) -> Z80<Box<crate::memory::Memory>, Box<crate::z80::test_utils::TestIo>> {',
        'fn create_z80(\n    program: &[u8],\n) -> crate::z80::test_utils::TestZ80 {'
    ),
    (
        'fn create_z80(program: &[u8]) -> Z80<Memory, TestIo> {',
        'fn create_z80(program: &[u8]) -> crate::z80::test_utils::TestZ80 {'
    ),
    (
        'fn create_z80(program: &[u8]) -> Z80<Box<Memory>, Box<TestIo>> {',
        'fn create_z80(program: &[u8]) -> crate::z80::test_utils::TestZ80 {'
    ),
    (
        'fn z80_setup() -> Z80<crate::memory::Memory, crate::z80::test_utils::TestIo> {',
        'fn z80_setup() -> crate::z80::test_utils::TestZ80 {'
    ),
    (
        'Z80::new(m, crate::z80::test_utils::TestIo::default())',
        'let cpu = Z80::new();\n    crate::z80::test_utils::TestZ80::new(cpu, m, crate::z80::test_utils::TestIo::default())'
    ),
    (
        'Z80::new(memory, crate::z80::test_utils::TestIo::default())',
        'let cpu = Z80::new();\n    crate::z80::test_utils::TestZ80::new(cpu, memory, crate::z80::test_utils::TestIo::default())'
    ),
    (
        'Z80::new(memory, TestIo::default())',
        'let cpu = Z80::new();\n    crate::z80::test_utils::TestZ80::new(cpu, memory, TestIo::default())'
    ),
]

gaps_old = """    Z80::new(
        Box::new(m),
        Box::new(crate::z80::test_utils::TestIo::default()),
    )"""
gaps_new = """    let cpu = Z80::new();
    crate::z80::test_utils::TestZ80::new(cpu, m, crate::z80::test_utils::TestIo::default())"""

ex_old = "Z80::new(Box::new(memory), Box::new(TestIo::default()))"
ex_new = "let cpu = Z80::new();\n    crate::z80::test_utils::TestZ80::new(cpu, memory, TestIo::default())"

for filepath in files:
    if filepath == 'src/z80/tests.rs': continue
    if filepath == 'src/z80/tests_alu.rs': continue
    if filepath == 'src/z80/tests_reset.rs': continue

    with open(filepath, 'r') as f:
        content = f.read()

    original_content = content

    for old, new in patterns:
        content = content.replace(old, new)

    content = content.replace(gaps_old, gaps_new)
    content = content.replace(ex_old, ex_new)

    snap_old = 'fn snapshot_memory<M: MemoryInterface, I: crate::memory::IoInterface>(\n    z80: &mut Z80<M, I>,\n) -> Vec<u8> {'
    snap_new = 'fn snapshot_memory(z80: &mut crate::z80::test_utils::TestZ80) -> Vec<u8> {'
    content = content.replace(snap_old, snap_new)

    if content != original_content:
        print(f"Updating {filepath}")
        with open(filepath, 'w') as f:
            f.write(content)
