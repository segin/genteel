//! Z80 Bus Adapter for Genesis Sound System
//!
//! Routes Z80 memory accesses to the appropriate Genesis components:
//! - 0000h-1FFFh: Z80 Sound RAM (8KB)
//! - 4000h-4003h: YM2612 FM Chip
//! - 6000h: Bank Register (sets 32KB window into 68k memory)
//! - 7F11h: SN76489 PSG
//! - 8000h-FFFFh: Banked 68k Memory (32KB window)

use super::{MemoryInterface, SharedBus};

/// Z80 Bus adapter that routes memory accesses to Genesis components
#[derive(Debug)]
pub struct Z80Bus {
    /// Reference to the main Genesis bus
    bus: SharedBus,
    
    /// Bank register: upper 9 bits of 68k address for $8000-$FFFF window
    /// When Z80 accesses $8000-$FFFF, the effective address is:
    /// (bank_register << 15) | (z80_addr & 0x7FFF)
    bank_register: u32,
}

impl Z80Bus {
    /// Create a new Z80 bus adapter
    pub fn new(bus: SharedBus) -> Self {
        Self {
            bus,
            bank_register: 0,
        }
    }
    
    /// Set the bank register (called on write to $6000)
    /// The value written becomes the upper bits of the 68k address
    pub fn set_bank(&mut self, value: u8) {
        // Bank register accumulates bits: each write shifts in one bit
        // The 9-bit bank value selects which 32KB page of 68k memory to map
        // Bits are written LSB first to $6000
        self.bank_register = ((self.bank_register >> 1) | ((value as u32 & 1) << 23)) & 0xFF8000;
    }
    
    /// Reset bank register to 0
    pub fn reset_bank(&mut self) {
        self.bank_register = 0;
    }
}

impl MemoryInterface for Z80Bus {
    fn read_byte(&mut self, address: u32) -> u8 {
        let addr = address as u16;
        
        match addr {
            // Z80 Sound RAM: 0000h-1FFFh
            0x0000..=0x1FFF => {
                self.bus.bus.borrow().z80_ram[addr as usize]
            }
            
            // Reserved: 2000h-3FFFh
            0x2000..=0x3FFF => 0xFF,
            
            // YM2612: 4000h-4003h
            0x4000..=0x4003 => {
                self.bus.bus.borrow().apu.fm.read((addr & 3) as u8)
            }
            
            // Reserved: 4004h-5FFFh
            0x4004..=0x5FFF => 0xFF,
            
            // Bank register area: 6000h-7F10h (reads return FF)
            0x6000..=0x7F10 => 0xFF,
            
            // PSG: 7F11h (write-only, reads return FF)
            0x7F11 => 0xFF,
            
            // Reserved: 7F12h-7FFFh
            0x7F12..=0x7FFF => 0xFF,
            
            // Banked 68k memory: 8000h-FFFFh
            0x8000..=0xFFFF => {
                let effective_addr = self.bank_register | ((addr as u32) & 0x7FFF);
                self.bus.bus.borrow_mut().read_byte(effective_addr)
            }
        }
    }
    
    fn write_byte(&mut self, address: u32, value: u8) {
        let addr = address as u16;
        
        match addr {
            // Z80 Sound RAM: 0000h-1FFFh
            0x0000..=0x1FFF => {
                self.bus.bus.borrow_mut().z80_ram[addr as usize] = value;
            }
            
            // Reserved: 2000h-3FFFh
            0x2000..=0x3FFF => {}
            
            // YM2612: 4000h-4003h
            0x4000..=0x4003 => {
                let port = (addr & 2) >> 1;  // 0 for 4000/4001, 1 for 4002/4003
                let is_data = (addr & 1) != 0;
                
                if is_data {
                    self.bus.bus.borrow_mut().apu.fm.write_data(port as u8, value);
                } else {
                    self.bus.bus.borrow_mut().apu.fm.write_address(port as u8, value);
                }
            }
            
            // Reserved: 4004h-5FFFh
            0x4004..=0x5FFF => {}
            
            // Bank register: 6000h
            0x6000..=0x60FF => {
                self.set_bank(value);
            }
            
            // Reserved: 6100h-7F10h
            0x6100..=0x7F10 => {}
            
            // PSG: 7F11h
            0x7F11 => {
                self.bus.bus.borrow_mut().apu.psg.write(value);
            }
            
            // Reserved: 7F12h-7FFFh
            0x7F12..=0x7FFF => {}
            
            // Banked 68k memory: 8000h-FFFFh
            0x8000..=0xFFFF => {
                let effective_addr = self.bank_register | ((addr as u32) & 0x7FFF);
                self.bus.bus.borrow_mut().write_byte(effective_addr, value);
            }
        }
    }
    
    fn read_word(&mut self, address: u32) -> u16 {
        let hi = self.read_byte(address) as u16;
        let lo = self.read_byte(address.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }
    
    fn write_word(&mut self, address: u32, value: u16) {
        self.write_byte(address, (value >> 8) as u8);
        self.write_byte(address.wrapping_add(1), value as u8);
    }
    
    fn read_long(&mut self, address: u32) -> u32 {
        let hi = self.read_word(address) as u32;
        let lo = self.read_word(address.wrapping_add(2)) as u32;
        (hi << 16) | lo
    }
    
    fn write_long(&mut self, address: u32, value: u32) {
        self.write_word(address, (value >> 16) as u16);
        self.write_word(address.wrapping_add(2), value as u16);
    }
    
    fn read_size(&mut self, address: u32, size: crate::cpu::decoder::Size) -> u32 {
        match size {
            crate::cpu::decoder::Size::Byte => self.read_byte(address) as u32,
            crate::cpu::decoder::Size::Word => self.read_word(address) as u32,
            crate::cpu::decoder::Size::Long => self.read_long(address),
        }
    }
    
    fn write_size(&mut self, address: u32, value: u32, size: crate::cpu::decoder::Size) {
        match size {
            crate::cpu::decoder::Size::Byte => self.write_byte(address, value as u8),
            crate::cpu::decoder::Size::Word => self.write_word(address, value as u16),
            crate::cpu::decoder::Size::Long => self.write_long(address, value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::cell::RefCell;
    use crate::memory::bus::Bus;
    
    fn create_test_z80_bus() -> Z80Bus {
        let bus = Rc::new(RefCell::new(Bus::new()));
        Z80Bus::new(SharedBus::new(bus))
    }
    
    #[test]
    fn test_z80_ram_read_write() {
        let mut z80_bus = create_test_z80_bus();
        
        z80_bus.write_byte(0x0000, 0x42);
        assert_eq!(z80_bus.read_byte(0x0000), 0x42);
        
        z80_bus.write_byte(0x1FFF, 0xAB);
        assert_eq!(z80_bus.read_byte(0x1FFF), 0xAB);
    }
    
    #[test]
    fn test_bank_register() {
        let mut z80_bus = create_test_z80_bus();
        
        // Initially bank is 0
        assert_eq!(z80_bus.bank_register, 0);
        
        // Write to bank register (bit-by-bit shifting)
        z80_bus.write_byte(0x6000, 0x01);  // Shift in 1
        assert_ne!(z80_bus.bank_register, 0);
    }
    
    #[test]
    fn test_reserved_reads_ff() {
        let mut z80_bus = create_test_z80_bus();
        
        // Reserved areas should return 0xFF
        assert_eq!(z80_bus.read_byte(0x2000), 0xFF);
        assert_eq!(z80_bus.read_byte(0x3FFF), 0xFF);
        assert_eq!(z80_bus.read_byte(0x7F11), 0xFF);  // PSG is write-only
    }
}
