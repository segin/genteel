use crate::memory::IoInterface;

#[derive(Debug, Default)]
pub struct TestIo;

impl IoInterface for TestIo {
    fn read_port(&mut self, _port: u16) -> u8 {
        0
    }

    fn write_port(&mut self, _port: u16, _value: u8) {}
}
