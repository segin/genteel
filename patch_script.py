import re

with open("src/main.rs", "r") as f:
    content = f.read()

# Replace struct definition
struct_from = """struct BusGdbMemory {
    bus: Rc<RefCell<Bus>>,
}
impl GdbMemory for BusGdbMemory {"""

struct_to = """struct BusGdbMemory<'a> {
    bus: &'a RefCell<Bus>,
}
impl<'a> GdbMemory for BusGdbMemory<'a> {"""

content = content.replace(struct_from, struct_to)

# Replace instantiations
inst_from = """let mut mem_access = BusGdbMemory {
                bus: self.bus.clone(),
            };"""
inst_to = """let mut mem_access = BusGdbMemory {
                bus: &self.bus,
            };"""

content = content.replace(inst_from, inst_to)

inst2_from = """let mut mem_access = BusGdbMemory {
                    bus: self.bus.clone(),
                };"""
inst2_to = """let mut mem_access = BusGdbMemory {
                    bus: &self.bus,
                };"""
content = content.replace(inst2_from, inst2_to)

with open("src/main.rs", "w") as f:
    f.write(content)
