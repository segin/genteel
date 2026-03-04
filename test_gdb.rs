use std::rc::Rc;
use std::cell::RefCell;

struct Bus {}
impl Bus {
    fn read_byte(&mut self, _addr: u32) -> u8 { 0 }
    fn write_byte(&mut self, _addr: u32, _value: u8) {}
}

trait GdbMemory {
    fn read_byte(&mut self, addr: u32) -> u8;
    fn write_byte(&mut self, addr: u32, value: u8);
}

struct BusGdbMemory<'a> {
    bus: &'a Rc<RefCell<Bus>>,
}
impl<'a> GdbMemory for BusGdbMemory<'a> {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.bus.borrow_mut().read_byte(addr)
    }
    fn write_byte(&mut self, addr: u32, value: u8) {
        self.bus.borrow_mut().write_byte(addr, value);
    }
}

struct Emulator {
    gdb: Option<String>,
    bus: Rc<RefCell<Bus>>,
}
impl Emulator {
    fn poll_gdb(&mut self) {
        let Some(gdb) = &mut self.gdb else { return };
        let mut mem_access = BusGdbMemory {
            bus: &self.bus,
        };
        // Use mem_access
        let _ = mem_access.read_byte(0);
        gdb.push_str("test"); // mutate gdb
    }
}
fn main() {}
