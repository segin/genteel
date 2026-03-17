import re

with open("src/memory/z80_bus.rs", "r") as f:
    content = f.read()

pattern = re.compile(r"fn read_byte\(&mut self, address: u32\) -> u8 \{.*?\n    \}\n\n    fn write_byte\(&mut self, address: u32, value: u8\) \{.*?\n    \}", re.DOTALL)
replacement = """fn read_byte(&mut self, address: u32) -> u8 {
        Self::read_byte_from_bus(&mut self.bus.bus.borrow_mut(), address)
    }

    fn write_byte(&mut self, address: u32, value: u8) {
        Self::write_byte_to_bus(&mut self.bus.bus.borrow_mut(), address, value)
    }"""

replaced = pattern.sub(replacement, content)

with open("src/memory/z80_bus.rs", "w") as f:
    f.write(replaced)
