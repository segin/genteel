import glob
import re

files = glob.glob('src/z80/tests*.rs')

for filepath in files:
    if filepath == 'src/z80/tests.rs': continue
    if filepath == 'src/z80/tests_alu.rs': continue
    if filepath == 'src/z80/tests_reset.rs': continue # Manual fix

    with open(filepath, 'r') as f:
        content = f.read()

    original_content = content

    # Update return signatures
    content = re.sub(
        r'fn\s+(\w+)\s*\(\s*program:\s*&\[u8\]\s*\)\s*->\s*Z80\s*<[^>]+>\s*\{',
        r'fn \1(program: &[u8]) -> crate::z80::test_utils::TestZ80 {',
        content,
        flags=re.DOTALL
    )

    content = re.sub(
        r'fn\s+z80_setup\s*\(\s*\)\s*->\s*Z80\s*<[^>]+>\s*\{',
        r'fn z80_setup() -> crate::z80::test_utils::TestZ80 {',
        content,
        flags=re.DOTALL
    )

    # Replace Z80::new(...) at the end of function (return value)
    # We look for Z80::new(...) }

    def replacer(match):
        args = match.group(1)
        return f'let cpu = Z80::new();\n    crate::z80::test_utils::TestZ80::new(cpu, {args})'

    # Match Z80::new(...) followed by optional whitespace and closing brace
    content = re.sub(r'Z80::new\s*\((.*)\)\s*\}', lambda m: replacer(m) + '\n}', content, flags=re.DOTALL)

    # Fix Box usage
    content = content.replace('Box::new(m)', 'm')
    content = content.replace('Box::new(memory)', 'memory')
    content = content.replace('Box::new(crate::z80::test_utils::TestIo::default())', 'crate::z80::test_utils::TestIo::default()')
    content = content.replace('Box::new(TestIo::default())', 'TestIo::default()')

    # Update snapshot_memory
    if 'snapshot_memory' in content:
        content = re.sub(
            r'fn\s+snapshot_memory.*z80:\s*&mut\s*Z80\s*<[^>]+>.*->\s*Vec<u8>\s*\{',
            r'fn snapshot_memory(z80: &mut crate::z80::test_utils::TestZ80) -> Vec<u8> {',
            content,
            flags=re.DOTALL
        )

    if content != original_content:
        print(f"Updating {filepath}")
        with open(filepath, 'w') as f:
            f.write(content)
