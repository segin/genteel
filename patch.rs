use std::fs;

fn main() {
    let content = fs::read_to_string("src/memory/z80_bus.rs").unwrap();
    let re = regex::Regex::new(r"(?s)fn read_byte\(&mut self, address: u32\) -> u8 \{.*?\n    \}\n\n    fn write_byte\(&mut self, address: u32, value: u8\) \{.*?\n    \}").unwrap();
    let replaced = re.replace(&content, "fn read_byte(&mut self, address: u32) -> u8 {\n        Self::read_byte_from_bus(&mut self.bus.bus.borrow_mut(), address)\n    }\n\n    fn write_byte(&mut self, address: u32, value: u8) {\n        Self::write_byte_to_bus(&mut self.bus.bus.borrow_mut(), address, value)\n    }");
    fs::write("src/memory/z80_bus.rs", replaced.as_ref()).unwrap();
}
