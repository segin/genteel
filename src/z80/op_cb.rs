use crate::memory::{MemoryInterface, Z80Interface};
use crate::z80::{flags, Z80};

pub trait CbOps {
    fn execute_cb_prefix<T: Z80Interface>(&mut self, bus: &mut T) -> u8;
    fn execute_indexed_cb<T: Z80Interface>(&mut self, bus: &mut T, opcode: u8, addr: u16) -> u8;
}

impl CbOps for Z80 {
    fn execute_cb_prefix<T: Z80Interface>(&mut self, bus: &mut T) -> u8 {
        let opcode = self.fetch_byte(bus);
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;

        let val = self.get_reg(bus, z);

        match x {
            0 => {
                // Rotate/shift
                let result = cb_rotate_shift(self, val, y);
                self.set_reg(bus, z, result);
                if z == 6 {
                    15
                } else {
                    8
                }
            }
            1 => {
                // BIT y, r
                cb_bit(self, val, y);

                if z != 6 {
                    let f = self.f & !(flags::X_FLAG | flags::Y_FLAG);
                    self.f = f | (val & (flags::X_FLAG | flags::Y_FLAG));
                } else {
                    // For (HL), X/Y come from MEMPTR (WZ) high byte.
                    let h_memptr = (self.memptr >> 8) as u8;
                    let f = self.f & !(flags::X_FLAG | flags::Y_FLAG);
                    self.f = f | (h_memptr & (flags::X_FLAG | flags::Y_FLAG));
                }

                if z == 6 {
                    12
                } else {
                    8
                }
            }
            2 => {
                // RES y, r
                let result = cb_res(val, y);
                self.set_reg(bus, z, result);
                if z == 6 {
                    15
                } else {
                    8
                }
            }
            3 => {
                // SET y, r
                let result = cb_set(val, y);
                self.set_reg(bus, z, result);
                if z == 6 {
                    15
                } else {
                    8
                }
            }
            _ => 8,
        }
    }

    fn execute_indexed_cb<T: Z80Interface>(&mut self, bus: &mut T, opcode: u8, addr: u16) -> u8 {
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let val = self.read_byte(bus, addr);

        match x {
            0 => {
                // Rotate/shift
                let result = cb_rotate_shift(self, val, y);
                self.write_byte(bus, addr, result);
                if z != 6 {
                    self.set_reg(bus, z, result);
                }
                23
            }
            1 => {
                // BIT y, (IX/IY+d)
                cb_bit(self, val, y);

                // X/Y from High Byte of EA
                let h_ea = (addr >> 8) as u8;
                let f = self.f & !(flags::X_FLAG | flags::Y_FLAG);
                self.f = f | (h_ea & (flags::X_FLAG | flags::Y_FLAG));
                20
            }
            2 => {
                // RES y, (IX/IY+d)
                let result = cb_res(val, y);
                self.write_byte(bus, addr, result);
                if z != 6 {
                    self.set_reg(bus, z, result);
                }
                23
            }
            3 => {
                // SET y, (IX/IY+d)
                let result = cb_set(val, y);
                self.write_byte(bus, addr, result);
                if z != 6 {
                    self.set_reg(bus, z, result);
                }
                23
            }
            _ => 20,
        }
    }
}

fn cb_rotate_shift(cpu: &mut Z80, val: u8, y: u8) -> u8 {
    let result = match y {
        0 => {
            // RLC
            let carry = (val & 0x80) != 0;
            cpu.set_flag(flags::CARRY, carry);
            (val << 1) | if carry { 1 } else { 0 }
        }
        1 => {
            // RRC
            let carry = (val & 0x01) != 0;
            cpu.set_flag(flags::CARRY, carry);
            (val >> 1) | if carry { 0x80 } else { 0 }
        }
        2 => {
            // RL
            let old_carry = cpu.get_flag(flags::CARRY);
            let carry = (val & 0x80) != 0;
            cpu.set_flag(flags::CARRY, carry);
            (val << 1) | if old_carry { 1 } else { 0 }
        }
        3 => {
            // RR
            let old_carry = cpu.get_flag(flags::CARRY);
            let carry = (val & 0x01) != 0;
            cpu.set_flag(flags::CARRY, carry);
            (val >> 1) | if old_carry { 0x80 } else { 0 }
        }
        4 => {
            // SLA
            let carry = (val & 0x80) != 0;
            cpu.set_flag(flags::CARRY, carry);
            val << 1
        }
        5 => {
            // SRA
            let carry = (val & 0x01) != 0;
            cpu.set_flag(flags::CARRY, carry);
            (val >> 1) | (val & 0x80)
        }
        6 => {
            // SLL (undocumented)
            let carry = (val & 0x80) != 0;
            cpu.set_flag(flags::CARRY, carry);
            (val << 1) | 1
        }
        7 => {
            // SRL
            let carry = (val & 0x01) != 0;
            cpu.set_flag(flags::CARRY, carry);
            val >> 1
        }
        _ => val,
    };
    cpu.set_flag(flags::HALF_CARRY, false);
    cpu.set_flag(flags::ADD_SUB, false);
    cpu.set_sz_flags(result);
    cpu.set_parity_flag(result);
    result
}

fn cb_bit(cpu: &mut Z80, val: u8, bit: u8) {
    let b = (val >> bit) & 1;
    cpu.set_flag(flags::ZERO, b == 0);
    cpu.set_flag(flags::HALF_CARRY, true);
    cpu.set_flag(flags::ADD_SUB, false);
}

fn cb_res(val: u8, bit: u8) -> u8 {
    val & !(1 << bit)
}

fn cb_set(val: u8, bit: u8) -> u8 {
    val | (1 << bit)
}
