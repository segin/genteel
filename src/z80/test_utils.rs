use crate::memory::IoInterface;

#[derive(Debug, Default)]
<<<<<<< HEAD
pub struct TestIo {}
=======
pub struct TestIo;
>>>>>>> main

impl IoInterface for TestIo {
    fn read_port(&mut self, _port: u16) -> u8 {
        0
    }

<<<<<<< HEAD
    fn write_port(&mut self, _port: u16, _value: u8) {
        // No-op
    }
=======
    fn write_port(&mut self, _port: u16, _value: u8) {}
>>>>>>> main
}
