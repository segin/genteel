use crate::memory::Memory;

#[derive(Debug)]
pub struct Z80 {
    // Main registers
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,

    // Alternate registers
    pub a_prime: u8,
    pub f_prime: u8,
    pub b_prime: u8,
    pub c_prime: u8,
    pub d_prime: u8,
    pub e_prime: u8,
    pub h_prime: u8,
    pub l_prime: u8,

    // Index registers
    pub ix: u16,
    pub iy: u16,

    // Stack Pointer and Program Counter
    pub sp: u16,
    pub pc: u16,

    // Interrupt Vector and Memory Refresh
    pub i: u8,
    pub r: u8,

    // Memory
    memory: Memory,
}

impl Z80 {
    pub fn new(memory: Memory) -> Self {
        Self {
            a: 0, f: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
            a_prime: 0, f_prime: 0, b_prime: 0, c_prime: 0, d_prime: 0, e_prime: 0, h_prime: 0, l_prime: 0,
            ix: 0, iy: 0,
            sp: 0, pc: 0,
            i: 0, r: 0,
            memory,
        }
    }

    // Register pair getters/setters
    pub fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    pub fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }

    pub fn step(&mut self) {
        let opcode = self.memory.data[self.pc as usize];
        self.pc += 1;
        
        // For now, only implement a few opcodes
        if opcode == 0x00 { // NOP
            // NOP does nothing
        } else if opcode == 0x36 { // LD (HL), n
            let n = self.memory.data[self.pc as usize];
            self.pc += 1;
            let hl = self.hl();
            self.memory.data[hl as usize] = n;
        } else if opcode == 0x78 { // LD A, B
            self.a = self.b;
        } else {
            // Unimplemented instruction
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

    #[test]
    fn test_nop() {
        let mut memory = Memory::new(1024);
        memory.data[0] = 0x00; // NOP

        let mut z80 = Z80::new(memory);
        z80.step();
        
        assert_eq!(z80.pc, 1); // PC should advance by 1
    }

    #[test]
    fn test_ld_hl_n() {
        let mut memory = Memory::new(1024);
        memory.data[0] = 0x36; // LD (HL), n
        memory.data[1] = 0xAB; // n = 0xAB

        let mut z80 = Z80::new(memory);
        z80.set_hl(0x0200);

        z80.step();

        assert_eq!(z80.pc, 2);
        assert_eq!(z80.memory.data[0x0200], 0xAB);
    }

    #[test]
    fn test_ld_a_b() {
        let memory = Memory::new(1024);
        let mut z80 = Z80::new(memory);
        z80.b = 0x42;
        z80.a = 0;

        // Manually place opcode in memory for this test. A bit artificial but works.
        z80.memory.data[0] = 0x78; // LD A, B

        z80.step();

        assert_eq!(z80.a, 0x42);
        assert_eq!(z80.pc, 1);
    }
}
