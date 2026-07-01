//! Tests for the EverDrive / SSF bank-switching mapper (4MB RAM).

use super::bus::Bus;
use super::everdrive::{EverdriveMapper, BANK_SIZE, NUM_SLOTS};
use proptest::prelude::*;

/// Address of register controlling slot `n`'s bank ($A130F1 + 2n).
fn bank_reg(slot: usize) -> u32 {
    0xA130F1 + (slot as u32) * 2
}

/// $A130F0 control byte assembled from its flag bits.
fn control(protect: bool, x: bool, w: bool, l: bool) -> u8 {
    (u8::from(protect) << 7) | (u8::from(x) << 6) | (u8::from(w) << 5) | (u8::from(l) << 4)
}

/// Write the header system field that selects (or not) the mapper.
fn write_header(rom: &mut [u8], magic: &[u8]) {
    rom[0x100..0x100 + magic.len()].copy_from_slice(magic);
}

/// Build a ROM that activates the mapper, with each 512KB bank's first two bytes
/// marked by the bank index so banks are individually identifiable.
fn ssf_rom_banked(num_banks: usize) -> Vec<u8> {
    let mut rom = vec![0u8; num_banks * BANK_SIZE];
    for b in 0..num_banks {
        rom[b * BANK_SIZE] = b as u8;
        rom[b * BANK_SIZE + 1] = 0xB0 | (b as u8);
    }
    write_header(&mut rom, b"SEGA SSF");
    rom
}

/// Build a small SSF ROM (backed by the minimum 4MB window).
fn ssf_rom_small() -> Vec<u8> {
    let mut rom = vec![0u8; 0x4000];
    for (i, b) in rom.iter_mut().enumerate() {
        *b = (i & 0xFF) as u8;
    }
    write_header(&mut rom, b"SEGA SSF");
    rom
}

/// Build a normal (non-SSF) ROM.
fn normal_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x4000];
    for (i, b) in rom.iter_mut().enumerate() {
        *b = (i & 0xFF) as u8;
    }
    write_header(&mut rom, b"SEGA GENESIS    ");
    rom
}

#[test]
fn test_header_detection() {
    assert!(EverdriveMapper::rom_uses_mapper(&ssf_rom_small()));
    assert!(!EverdriveMapper::rom_uses_mapper(&normal_rom()));
    // Too short to contain the header magic.
    assert!(!EverdriveMapper::rom_uses_mapper(&[0u8; 0x104]));
}

#[test]
fn test_mapper_enabled_only_for_ssf() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_small());
    assert!(bus.everdrive.enabled);

    let mut bus2 = Bus::new();
    bus2.load_rom(&normal_rom());
    assert!(!bus2.everdrive.enabled);
}

#[test]
fn test_default_linear_mapping() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_banked(NUM_SLOTS));
    // Slot N defaults to bank N, so each slot base reads back its own index.
    for slot in 0..NUM_SLOTS {
        let addr = (slot as u32) * BANK_SIZE as u32;
        assert_eq!(bus.read_byte(addr), slot as u8, "slot {slot} default bank");
    }
}

#[test]
fn test_bank_switch() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_banked(NUM_SLOTS));

    // Point slot 1 at bank 3 and slot 7 at bank 0.
    bus.write_byte(bank_reg(1), 3);
    bus.write_byte(bank_reg(7), 0);

    assert_eq!(bus.read_byte(BANK_SIZE as u32), 3, "slot 1 -> bank 3");
    assert_eq!(bus.read_byte(7 * BANK_SIZE as u32), 0, "slot 7 -> bank 0");
    // Unswitched slots keep their default mapping.
    assert_eq!(bus.read_byte(2 * BANK_SIZE as u32), 2, "slot 2 unchanged");
}

#[test]
fn test_slot0_is_switchable() {
    // The extended SSF mapper allows the first bank to be switched like any other.
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_banked(NUM_SLOTS));
    bus.write_byte(bank_reg(0), 5);
    assert_eq!(bus.read_byte(0), 5, "slot 0 -> bank 5");
}

#[test]
fn test_bank_register_masks_to_5_bits() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_small());
    bus.write_byte(bank_reg(2), 0xFF);
    assert_eq!(bus.everdrive.banks[2], 0x1F);
}

#[test]
fn test_writes_protected_by_default() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_small());
    let before = bus.read_byte(0x1234);
    bus.write_byte(0x1234, before.wrapping_add(1));
    assert_eq!(
        bus.read_byte(0x1234),
        before,
        "write ignored while protected"
    );
}

#[test]
fn test_write_enable_ram_roundtrip() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_small());

    // Enable writes (protection + W bit).
    bus.write_byte(0xA130F0, control(true, false, true, false));
    assert!(bus.everdrive.write_enable);

    bus.write_byte(0x1234, 0xAB);
    assert_eq!(bus.read_byte(0x1234), 0xAB, "RAM byte round-trips");
}

#[test]
fn test_full_4mb_window_is_ram() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_banked(NUM_SLOTS));
    bus.write_byte(0xA130F0, control(true, false, true, false));

    // Write a distinct value into each of the eight 512KB slots and read back.
    for slot in 0..NUM_SLOTS {
        let addr = (slot as u32) * BANK_SIZE as u32 + 0x100;
        bus.write_byte(addr, 0x40 + slot as u8);
    }
    for slot in 0..NUM_SLOTS {
        let addr = (slot as u32) * BANK_SIZE as u32 + 0x100;
        assert_eq!(bus.read_byte(addr), 0x40 + slot as u8, "slot {slot} RAM");
    }
}

#[test]
fn test_control_protection_latch() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_small());

    // Without the protection bit the control write is ignored.
    bus.write_byte(0xA130F0, control(false, false, true, false));
    assert!(!bus.everdrive.write_enable, "control ignored without P bit");

    // With the protection bit it loads.
    bus.write_byte(0xA130F0, control(true, false, true, false));
    assert!(bus.everdrive.write_enable, "control loaded with P bit");
}

#[test]
fn test_control_x_and_l_bits() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_small());

    bus.write_byte(0xA130F0, control(true, true, true, true));
    assert!(bus.everdrive.mode_32x);
    assert!(bus.everdrive.write_enable);
    assert!(bus.everdrive.led);

    bus.write_byte(0xA130F0, control(true, false, false, false));
    assert!(!bus.everdrive.mode_32x);
    assert!(!bus.everdrive.write_enable);
    assert!(!bus.everdrive.led);
}

#[test]
fn test_non_ssf_keeps_sram_enable_register() {
    // For a normal cart, $A130F1 retains its SRAM-enable meaning.
    let mut bus = Bus::new();
    bus.load_rom(&normal_rom());

    bus.write_byte(0xA130F1, 0x01);
    assert!(bus.sram_enabled);
    bus.write_byte(0xA130F1, 0x00);
    assert!(!bus.sram_enabled);
    // The mapper stays inactive and its banks untouched.
    assert!(!bus.everdrive.enabled);
}

#[test]
fn test_word_and_long_reads_route_through_mapper() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_banked(NUM_SLOTS));

    // Word/long reads must agree with byte reads now that the ROM fast path is
    // bypassed for the mapper.
    for slot in 0..NUM_SLOTS {
        let addr = (slot as u32) * BANK_SIZE as u32;
        let b0 = bus.read_byte(addr);
        let b1 = bus.read_byte(addr + 1);
        let b2 = bus.read_byte(addr + 2);
        let b3 = bus.read_byte(addr + 3);
        assert_eq!(bus.read_word(addr), ((b0 as u16) << 8) | b1 as u16);
        assert_eq!(
            bus.read_long(addr),
            ((b0 as u32) << 24) | ((b1 as u32) << 16) | ((b2 as u32) << 8) | b3 as u32
        );
    }
}

#[test]
fn test_out_of_range_bank_reads_open_bus() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_small()); // backing memory is the minimum 4MB (banks 0..=7)
                                    // Bank 10 lies past the 4MB backing buffer.
    bus.write_byte(bank_reg(0), 10);
    assert_eq!(bus.read_byte(0), 0xFF, "out-of-range bank reads open bus");
}

#[test]
fn test_reset_restores_default_banks_but_keeps_memory() {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom_banked(NUM_SLOTS));

    // Mutate registers and RAM.
    bus.write_byte(0xA130F0, control(true, false, true, false));
    bus.write_byte(bank_reg(1), 4);
    bus.write_byte(0x1000, 0x99);

    bus.reset();

    // Registers are back to defaults...
    assert_eq!(bus.everdrive.banks, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(!bus.everdrive.write_enable);
    // ...but the mapper is still active and its backing memory intact.
    assert!(bus.everdrive.enabled);
    assert_eq!(bus.read_byte(BANK_SIZE as u32), 1, "slot 1 back to bank 1");
}

#[test]
fn test_serde_preserves_registers_and_rebuilds_memory() {
    let rom = ssf_rom_banked(NUM_SLOTS);
    let mut bus = Bus::new();
    bus.load_rom(&rom);
    bus.write_byte(0xA130F0, control(true, false, true, false));
    bus.write_byte(bank_reg(3), 6);

    // Serialize just the mapper and round-trip it.
    let json = serde_json::to_string(&bus.everdrive).unwrap();
    let mut restored: EverdriveMapper = serde_json::from_str(&json).unwrap();

    // Register state survives; volatile fields (mem/enabled) are reconstructed.
    assert_eq!(restored.banks[3], 6);
    assert!(restored.write_enable);
    assert!(!restored.enabled, "enabled is derived, not serialized");
    assert!(restored.mem.is_empty(), "backing memory is not serialized");

    // load_rom (which runs after deserialize) re-establishes the backing memory
    // without disturbing the restored register state.
    restored.load_rom(&rom);
    assert!(restored.enabled);
    assert_eq!(restored.banks[3], 6, "banks untouched by load_rom");
    assert_eq!(restored.read(3 * BANK_SIZE as u32), 6, "slot 3 -> bank 6");
}

proptest! {
    // Reduce cases: each builds a >=4MB backing buffer.
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// With writes enabled, a byte written anywhere in the window reads back.
    #[test]
    fn prop_ram_roundtrip(
        slot in 0usize..NUM_SLOTS,
        bank in 0u8..NUM_SLOTS as u8,
        off in 0u32..BANK_SIZE as u32,
        val in any::<u8>(),
    ) {
        let mut m = EverdriveMapper::new();
        m.load_rom(&[0u8; 0x1000]);   // minimum 4MB backing buffer
        m.write_enable = true;
        m.banks[slot] = bank;          // bank < 8 stays within the 4MB buffer
        let addr = (slot as u32) * BANK_SIZE as u32 + off;
        m.write(addr, val);
        prop_assert_eq!(m.read(addr), val);
    }

    /// While write-protected, writes never change what is read back.
    #[test]
    fn prop_write_protected_is_noop(
        addr in 0u32..(NUM_SLOTS as u32 * BANK_SIZE as u32),
        val in any::<u8>(),
    ) {
        let mut m = EverdriveMapper::new();
        m.load_rom(&[0u8; 0x1000]);
        // write_enable defaults to false
        let before = m.read(addr);
        m.write(addr, val);
        prop_assert_eq!(m.read(addr), before);
    }

    /// A bank register only ever latches its low 5 bits.
    #[test]
    fn prop_bank_register_masks(slot in 0usize..NUM_SLOTS, val in any::<u8>()) {
        let mut m = EverdriveMapper::new();
        m.enabled = true;
        m.write_register(bank_reg(slot), val);
        prop_assert_eq!(m.banks[slot], val & 0x1F);
    }
}
