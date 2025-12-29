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
    pub fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }

    pub fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }

    pub fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }

    pub fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }

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
        
        match opcode {
            0x00 => {}, // NOP
            0x01 => { // LD BC, nn (little-endian)
                let low = self.memory.data[self.pc as usize] as u16;
                let high = self.memory.data[(self.pc + 1) as usize] as u16;
                let nn = (high << 8) | low;
                self.pc += 2;
                self.set_bc(nn);
            },
            0x03 => { // INC BC
                let val = self.bc().wrapping_add(1);
                self.set_bc(val);
            },
            0x11 => { // LD DE, nn
                let nn = self.memory.read_word_le(self.pc as u32);
                self.pc += 2;
                self.set_de(nn);
            },
            0x13 => { // INC DE
                let val = self.de().wrapping_add(1);
                self.set_de(val);
            },
            0x21 => { // LD HL, nn
                let nn = self.memory.read_word_le(self.pc as u32);
                self.pc += 2;
                self.set_hl(nn);
            },
            0x23 => { // INC HL
                let val = self.hl().wrapping_add(1);
                self.set_hl(val);
            },
            0x31 => { // LD SP, nn
                let nn = self.memory.read_word_le(self.pc as u32);
                self.pc += 2;
                self.sp = nn;
            },
            0x33 => { // INC SP
                self.sp = self.sp.wrapping_add(1);
            },
            0x36 => { // LD (HL), n
                let n = self.memory.data[self.pc as usize];
                self.pc += 1;
                let hl = self.hl();
                self.memory.data[hl as usize] = n;
            },
            0x41 => { // LD B, C
                self.b = self.c;
            },
            0x78 => { // LD A, B
                self.a = self.b;
            },
            _ => {
                // Unimplemented instruction
            }
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
    fn test_ld_bc_nn() {
        let mut memory = Memory::new(1024);
        memory.data[0] = 0x01; // LD BC, nn
        memory.data[1] = 0x34; // Low byte
        memory.data[2] = 0x12; // High byte (Little Endian)

        let mut z80 = Z80::new(memory);
        z80.step();

        assert_eq!(z80.bc(), 0x1234);
        assert_eq!(z80.pc, 3);
    }

    #[test]
    fn test_inc_hl() {
        let mut memory = Memory::new(1024);
        memory.data[0] = 0x23; // INC HL

        let mut z80 = Z80::new(memory);
        z80.set_hl(0xFFFF);
        z80.step();

        assert_eq!(z80.hl(), 0x0000); // Check wrapping
        assert_eq!(z80.pc, 1);
    }

    #[test]
    fn test_ld_b_c() {
        let mut memory = Memory::new(1024);
        memory.data[0] = 0x41; // LD B, C

        let mut z80 = Z80::new(memory);
        z80.c = 0x55;
        z80.step();

        assert_eq!(z80.b, 0x55);
        assert_eq!(z80.pc, 1);
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
        let mut memory = Memory::new(1024);
        memory.data[0] = 0x78; // LD A, B
        
        let mut z80 = Z80::new(memory);
        z80.b = 0x42;
        z80.a = 0;

        z80.step();

        assert_eq!(z80.a, 0x42);
        assert_eq!(z80.pc, 1);
    }
}
