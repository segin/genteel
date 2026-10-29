//! Memory-map behaviour: cartridge SRAM and mappers, Z80 window, VDP port
//! decode, region, and state validation.

use super::bus::Bus;
use super::z80_bus::Z80Bus;
use super::{MemoryInterface, SharedBus};
use crate::debugger::Debuggable;
use crate::io::ConsoleRegion;
use proptest::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// ROM of `len` bytes (pattern data) with an SRAM header.
fn rom_with_sram(len: usize, kind: u8, start: u32, end: u32) -> Vec<u8> {
    let mut rom: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    rom[0x1B0] = b'R';
    rom[0x1B1] = b'A';
    rom[0x1B2] = kind;
    rom[0x1B3] = 0x20;
    rom[0x1B4..0x1B8].copy_from_slice(&start.to_be_bytes());
    rom[0x1B8..0x1BC].copy_from_slice(&end.to_be_bytes());
    rom[0x1F0..0x1F4].copy_from_slice(b"U   ");
    rom
}

fn sonic3_like_rom() -> Vec<u8> {
    rom_with_sram(0x200000, 0xF8, 0x200001, 0x203FFF)
}

#[test]
fn odd_lane_sram_round_trips() {
    let mut bus = Bus::new();
    bus.load_rom(&sonic3_like_rom());
    assert!(
        bus.sram.mapped,
        "2 MiB ROM: SRAM above ROM is mapped at power-on"
    );
    bus.write_byte(0x200001, 0x5A);
    bus.write_byte(0x203FFF, 0xA5);
    assert_eq!(bus.read_byte(0x200001), 0x5A);
    assert_eq!(bus.read_byte(0x203FFF), 0xA5);
    // Untouched lane keeps its fill.
    assert_eq!(bus.read_byte(0x200000), 0xFF);
}

#[test]
fn word_and_long_reads_see_sram() {
    let mut bus = Bus::new();
    bus.load_rom(&rom_with_sram(0x100000, 0xE0, 0x200000, 0x20FFFF));
    bus.write_word(0x200010, 0x1234);
    bus.write_long(0x200020, 0xCAFEBABE);
    assert_eq!(bus.read_word(0x200010), 0x1234);
    assert_eq!(bus.read_byte(0x200010), 0x12);
    assert_eq!(bus.read_byte(0x200011), 0x34);
    assert_eq!(bus.read_long(0x200020), 0xCAFEBABE);
    assert_eq!(bus.read_long(0x200010), 0x1234FFFF);
}

#[test]
fn reset_preserves_sram_and_restores_power_on_mapping() {
    let mut bus = Bus::new();
    bus.load_rom(&sonic3_like_rom());
    bus.write_byte(0x200001, 0x42);
    bus.write_byte(0xA130F1, 0x00); // unmap
    assert!(!bus.sram.mapped);
    bus.reset();
    assert!(bus.sram.mapped, "mapping re-derived from the header");
    assert_eq!(bus.read_byte(0x200001), 0x42, "contents survive reset");
}

#[test]
fn large_rom_keeps_rom_at_2mb_until_sram_enabled() {
    let rom = rom_with_sram(0x300000, 0xF8, 0x200001, 0x203FFF);
    let mut bus = Bus::new();
    bus.load_rom(&rom);
    assert!(!bus.sram.mapped);
    assert_eq!(bus.read_byte(0x200001), rom[0x200001]);
    assert_eq!(
        bus.read_word(0x200000),
        u16::from_be_bytes([rom[0x200000], rom[0x200001]])
    );

    bus.write_byte(0xA130F1, 0x01);
    bus.write_byte(0x200001, 0x77);
    assert_eq!(bus.read_byte(0x200001), 0x77);

    bus.write_byte(0xA130F1, 0x00);
    assert_eq!(bus.read_byte(0x200001), rom[0x200001]);

    // Reset hides SRAM again; its contents stay.
    bus.write_byte(0xA130F1, 0x01);
    bus.reset();
    assert!(!bus.sram.mapped);
    bus.write_byte(0xA130F1, 0x01);
    assert_eq!(bus.read_byte(0x200001), 0x77);
}

#[test]
fn write_protect_discards_sram_writes() {
    let mut bus = Bus::new();
    bus.load_rom(&sonic3_like_rom());
    bus.write_byte(0x200001, 0x11);
    bus.write_byte(0xA130F1, 0x03); // mapped + write protected
    bus.write_byte(0x200001, 0x22);
    bus.write_word(0x200000, 0x3333);
    assert_eq!(bus.read_byte(0x200001), 0x11);
    bus.write_byte(0xA130F1, 0x01);
    bus.write_byte(0x200001, 0x22);
    assert_eq!(bus.read_byte(0x200001), 0x22);
}

#[test]
fn headerless_small_rom_gets_default_sram() {
    let mut bus = Bus::new();
    bus.load_rom(&vec![0u8; 0x80000]);
    assert!(bus.sram.present && bus.sram.mapped);
    bus.write_byte(0x200000, 0x99);
    assert_eq!(bus.read_byte(0x200000), 0x99);
}

#[test]
fn header_with_overflowing_range_loads() {
    let mut bus = Bus::new();
    bus.load_rom(&rom_with_sram(0x1000, 0xE0, 0, 0xFFFF_FFFF));
    assert!(bus.sram.present);
}

#[test]
fn sega_mapper_pages_banks_into_slots_one_to_seven() {
    let rom: Vec<u8> = (0..0x500000usize).map(|i| (i >> 19) as u8).collect();
    let mut bus = Bus::new();
    bus.load_rom(&rom);
    assert!(bus.rom_mapper.enabled);
    assert_eq!(bus.read_byte(0x080000), 1);
    bus.write_byte(0xA130F3, 9); // slot 1 -> bank 9
    assert_eq!(bus.read_byte(0x080000), 9);
    assert_eq!(bus.read_word(0x0FFFFE), 0x0909);
    bus.write_byte(0xA130FF, 8); // slot 7 -> bank 8
    assert_eq!(bus.read_long(0x380000), 0x08080808);
    // Slot 0 is fixed; $A130F1 is the SRAM register.
    bus.write_byte(0xA130F1, 0x05);
    assert_eq!(bus.read_byte(0x000000), 0);
    // Linear view above 4 MiB.
    assert_eq!(bus.read_byte(0x480000), 9);
    // DMA sees the paged bank.
    assert_eq!(bus.dma_source_word(0x080000), 0x0909);
    bus.reset();
    assert_eq!(bus.read_byte(0x080000), 1);
}

#[test]
fn small_rom_has_no_sega_mapper() {
    let mut bus = Bus::new();
    bus.load_rom(&vec![0u8; 0x100000]);
    assert!(!bus.rom_mapper.enabled);
}

#[test]
fn dma_source_sees_mapped_sram() {
    let mut bus = Bus::new();
    bus.load_rom(&rom_with_sram(0x300000, 0xE0, 0x200000, 0x20FFFF));
    let rom_word = bus.dma_source_word(0x200000);
    bus.write_byte(0xA130F1, 0x01);
    bus.write_word(0x200000, 0xBEEF);
    assert_eq!(bus.dma_source_word(0x200000), 0xBEEF);
    bus.write_byte(0xA130F1, 0x00);
    assert_eq!(bus.dma_source_word(0x200000), rom_word);
}

#[test]
fn z80_window_word_read_duplicates_byte() {
    let mut bus = Bus::new();
    bus.write_byte(0xA11100, 0x01); // BUSREQ
    bus.write_byte(0xA11200, 0x01); // and out of reset: bus granted
    bus.z80_ram[0] = 0x12;
    bus.z80_ram[1] = 0x34;
    bus.z80_ram[2] = 0x56;
    assert_eq!(bus.read_word(0xA00000), 0x1212);
    assert_eq!(bus.read_long(0xA00000), 0x12125656);
}

#[test]
fn z80_window_word_write_stores_high_byte_only() {
    let mut bus = Bus::new();
    bus.write_byte(0xA11100, 0x01);
    bus.write_byte(0xA11200, 0x01);
    bus.z80_ram[1] = 0x99;
    bus.write_word(0xA00000, 0xABCD);
    assert_eq!(bus.z80_ram[0], 0xAB);
    assert_eq!(bus.z80_ram[1], 0x99);
    bus.write_long(0xA00010, 0x11223344);
    assert_eq!(&bus.z80_ram[0x10..0x14], &[0x11, 0x00, 0x33, 0x00]);
}

#[test]
fn z80_ram_is_mirrored_at_a02000() {
    let mut bus = Bus::new();
    bus.write_byte(0xA11100, 0x01);
    bus.write_byte(0xA11200, 0x01);
    bus.write_byte(0xA02005, 0x77);
    assert_eq!(bus.z80_ram[5], 0x77);
    assert_eq!(bus.read_byte(0xA02005), 0x77);
    assert_eq!(bus.read_byte(0xA00005), 0x77);
}

#[test]
fn z80_ram_needs_the_bus_granted_not_only_requested() {
    let mut bus = Bus::new();
    bus.z80_ram[0x10] = 0x5A;
    bus.write_byte(0xA11100, 0x01); // BUSREQ, Z80 still in reset
    assert_eq!(bus.read_byte(0xA00010), 0xFF, "no access while in reset");
    bus.write_byte(0xA00010, 0x11);
    assert_eq!(bus.z80_ram[0x10], 0x5A, "write dropped while in reset");

    bus.write_byte(0xA11200, 0x01); // release reset: bus granted
    assert_eq!(bus.read_byte(0xA00010), 0x5A);
    bus.write_byte(0xA00010, 0x11);
    assert_eq!(bus.z80_ram[0x10], 0x11);

    bus.write_byte(0xA11100, 0x00); // Z80 takes its bus back
    assert_eq!(bus.read_byte(0xA00010), 0xFF);
}

#[test]
fn busack_requires_request_and_reset_released() {
    let mut bus = Bus::new();
    bus.write_byte(0xA11100, 0x01);
    bus.write_byte(0xA11200, 0x00);
    assert_eq!(bus.read_byte(0xA11100) & 1, 1, "reset held: not granted");
    bus.write_byte(0xA11200, 0x01);
    assert_eq!(bus.read_byte(0xA11100) & 1, 0, "granted");
    bus.write_byte(0xA11100, 0x00);
    assert_eq!(bus.read_byte(0xA11100) & 1, 1);
}

#[test]
fn z80_reset_resets_ym2612() {
    let mut bus = Bus::new();
    bus.write_byte(0xA11200, 0x01);
    bus.write_byte(0xA04000, 0x30);
    bus.write_byte(0xA04001, 0x71);
    assert_eq!(bus.apu.fm.registers[0][0x30], 0x71);
    bus.write_byte(0xA11200, 0x00);
    assert_eq!(bus.apu.fm.registers[0][0x30], 0x00);
}

#[test]
fn vdp_ports_decode_identically_across_mirrors() {
    let mut bus = Bus::new();
    bus.write_word(0xC00100, 0x1234);
    assert_eq!(bus.vdp.last_data_write, 0x1234, "word write is not split");
    bus.write_long(0xD80000, 0x5678ABCD);
    assert_eq!(bus.vdp.last_data_write, 0xABCD);
    bus.write_byte(0xC80001, 0x42);
    assert_eq!(bus.vdp.last_data_write, 0x4242);

    // A5-A7 or A16-A18 set: not a VDP port.
    bus.write_word(0xC00020, 0x1111);
    bus.write_word(0xC10000, 0x2222);
    assert_eq!(bus.vdp.last_data_write, 0x4242);
    assert_eq!(bus.read_word(0xC00028), 0xFFFF);
}

#[test]
fn psg_answers_on_odd_bytes_and_words_of_its_range() {
    let mut bus = Bus::new();
    bus.write_byte(0xC00013, 0x90);
    assert_eq!(bus.apu.psg.tones[0].volume, 0);
    bus.write_byte(0xC00010, 0x9F); // even byte: ignored
    assert_eq!(bus.apu.psg.tones[0].volume, 0);
    bus.write_word(0xC00016, 0x009F);
    assert_eq!(bus.apu.psg.tones[0].volume, 0x0F);
}

#[test]
fn long_data_port_write_charges_stall_per_word() {
    let mut bus = Bus::new();
    bus.vdp.command.code = crate::vdp::VRAM_WRITE;
    bus.vdp.fifo_full = true;
    bus.write_long(0xC00000, 0x11112222);
    let stall = bus.take_pending_stall_cycles();
    assert_eq!(stall, 8, "two FIFO-full words");
}

#[test]
fn pad_data_writes_do_not_latch_hv_counter() {
    let mut bus = Bus::new();
    bus.write_word(0xC00004, 0x8002); // reg 0: M3 (HV latch enable)
    bus.write_byte(0xA10009, 0x40);
    bus.write_byte(0xA10003, 0x00);
    bus.write_byte(0xA10003, 0x40);
    assert!(bus.vdp.hv_latched.is_none());
}

/// Only a device driving the TH input (a light gun) latches the HV counter;
/// the 68k toggling TH on any port, as data or as direction, never does.
#[test]
fn software_th_toggles_never_latch_hv_counter() {
    let mut bus = Bus::new();
    bus.write_word(0xC00004, 0x8104); // reg 1: mode 5
    bus.write_word(0xC00004, 0x8002); // reg 0: M3 (HV latch enable)
    for (data, ctrl) in [
        (0xA10003, 0xA10009),
        (0xA10005, 0xA1000B),
        (0xA10007, 0xA1000D),
    ] {
        for (d, c) in [
            (0x00, 0x40),
            (0x40, 0x40),
            (0x00, 0x40),
            (0x00, 0x00),
            (0x40, 0xC0),
        ] {
            bus.write_byte(ctrl, c);
            bus.write_byte(data, d);
        }
    }
    bus.tick(7 * 1000);
    assert!(bus.vdp.hv_latched.is_none());
}

#[test]
fn bus_tick_drives_six_button_timeout() {
    let mut bus = Bus::new();
    bus.io
        .set_controller_type(1, crate::io::ControllerType::SixButton);
    bus.write_byte(0xA10003, 0x40);
    bus.write_byte(0xA10009, 0x40);
    bus.write_byte(0xA10003, 0x00);
    bus.write_byte(0xA10003, 0x40);
    assert_eq!(bus.io.port1.th_counter, 2);
    // 1.5 ms worth of 68k cycles, delivered in MCLK slices.
    for _ in 0..(crate::io::SIX_BUTTON_TIMEOUT_CYCLES / 100 + 1) {
        bus.tick(700);
    }
    assert_eq!(bus.io.port1.th_counter, 0);
}

#[test]
fn region_follows_header_and_override() {
    let mut rom = vec![0u8; 0x1000];
    rom[0x1F0] = b'E';
    let mut bus = Bus::new();
    bus.load_rom(&rom);
    assert_eq!(bus.read_byte(0xA10001), 0xE0);
    assert!(bus.vdp.is_pal, "PAL region runs 313-line PAL timing");

    rom[0x1F0] = b'J';
    bus.load_rom(&rom);
    assert_eq!(bus.read_byte(0xA10001), 0x20);
    assert!(!bus.vdp.is_pal);

    bus.set_region_override(Some(ConsoleRegion::Europe));
    assert_eq!(bus.read_byte(0xA10001), 0xE0);
    assert!(bus.vdp.is_pal);
    bus.set_region_override(None);
    assert_eq!(bus.read_byte(0xA10001), 0x20);
}

fn z80_bus() -> (Rc<RefCell<Bus>>, Z80Bus) {
    let bus = Rc::new(RefCell::new(Bus::new()));
    let z80 = Z80Bus::new(SharedBus::new(bus.clone()));
    (bus, z80)
}

#[test]
fn z80_reads_vdp_hv_counter_at_7f08() {
    let (bus, mut z80) = z80_bus();
    bus.borrow_mut().vdp.set_v_counter(100);
    let expected = bus.borrow_mut().read_byte(0xC00008);
    assert_eq!(z80.read_byte(0x7F08), expected);
    assert_eq!(z80.read_byte(0x7F20), 0xFF, "beyond the port block");
}

#[test]
fn z80_psg_writes_reach_every_mirror() {
    let (bus, mut z80) = z80_bus();
    for (addr, vol) in [
        (0x7F11u32, 0x90u8),
        (0x7F13, 0x91),
        (0x7F15, 0x92),
        (0x7F17, 0x93),
    ] {
        z80.write_byte(addr, vol);
        assert_eq!(
            bus.borrow().apu.psg.tones[0].volume,
            vol & 0x0F,
            "{:04X}",
            addr
        );
    }
}

#[test]
fn z80_bank_into_own_window_locks_up_until_reset() {
    let (bus, mut z80) = z80_bus();
    bus.borrow_mut().write_byte(0xA11200, 0x01);
    bus.borrow_mut().z80_bank_addr = 0xA00000;
    assert!(bus.borrow().z80_can_run());
    assert_eq!(z80.read_byte(0x8000), 0xFF);
    assert!(bus.borrow().z80_lockup);
    assert!(!bus.borrow().z80_can_run());
    bus.borrow_mut().write_byte(0xA11200, 0x00);
    bus.borrow_mut().write_byte(0xA11200, 0x01);
    assert!(bus.borrow().z80_can_run());
}

fn on_big_stack<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn write_state_rejects_wrong_ram_sizes() {
    on_big_stack(|| {
        let mut bus = Bus::new();
        bus.work_ram[0] = 0x55;
        let mut state = bus.read_state();
        state["work_ram"] = serde_json::json!([1, 2, 3]);
        state["z80_bus_request"] = serde_json::json!(true);
        bus.write_state(&state);
        assert_eq!(bus.work_ram.len(), 0x10000);
        assert!(!bus.z80_bus_request, "whole state rejected");

        let mut state = bus.read_state();
        state["z80_ram"] = serde_json::json!(vec![0u8; 0x2001]);
        bus.write_state(&state);
        assert_eq!(bus.z80_ram.len(), 0x2000);
    });
}

#[test]
fn deserialize_rejects_wrong_ram_sizes() {
    on_big_stack(|| {
        let bus = Bus::new();
        let mut value = serde_json::to_value(&bus).unwrap();
        assert!(serde_json::from_value::<Bus>(value.clone()).is_ok());
        value["z80_ram"] = serde_json::json!([0, 1]);
        assert!(serde_json::from_value::<Bus>(value).is_err());
    });
}

#[test]
fn serialized_bus_keeps_legacy_sram_keys() {
    on_big_stack(|| {
        let mut bus = Bus::new();
        bus.load_rom(&sonic3_like_rom());
        bus.write_byte(0x200001, 0x42);
        let value = serde_json::to_value(&bus).unwrap();
        for key in ["sram", "sram_enabled", "sram_start", "sram_end"] {
            assert!(value.get(key).is_some(), "{}", key);
        }
        let restored: Bus = serde_json::from_value(value).unwrap();
        assert_eq!(restored.sram.read_byte(0x200001), 0x42);
        assert!(restored.sram.mapped);
    });
}

proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
    /// Every byte/word access to $C00000-$DFFFFF decodes to a VDP port
    /// exactly when (addr & 0xE700E0) == 0xC00000.
    #[test]
    fn prop_vdp_mirror_decode(hi in 0u32..0x200000) {
        let base = (0xC00000 + hi) & !0x1F;
        let mut bus = Bus::new();
        let valid = base & 0xE700E0 == 0xC00000;
        bus.write_word(base, 0x1357);
        prop_assert_eq!(bus.vdp.last_data_write == 0x1357, valid);
        let hv = bus.read_word(base | 0x08);
        if !valid {
            prop_assert_eq!(hv, 0xFFFF);
        }
    }

    /// Z80 RAM is visible at both $A00000 and its $A02000 mirror.
    #[test]
    fn prop_z80_ram_mirror(off in 0u32..0x2000, v in any::<u8>()) {
        let mut bus = Bus::new();
        bus.write_byte(0xA11100, 0x01);
        bus.write_byte(0xA11200, 0x01);
        bus.write_byte(0xA02000 + off, v);
        prop_assert_eq!(bus.read_byte(0xA00000 + off), v);
    }

    /// SRAM byte, word and long views agree.
    #[test]
    fn prop_sram_access_sizes_agree(off in 0u32..0x3FF0, v in any::<u32>()) {
        let mut bus = Bus::new();
        bus.load_rom(&rom_with_sram(0x1000, 0xE0, 0x200000, 0x203FFF));
        let addr = 0x200000 + (off & !1);
        bus.write_long(addr, v);
        let bytes = [
            bus.read_byte(addr),
            bus.read_byte(addr + 1),
            bus.read_byte(addr + 2),
            bus.read_byte(addr + 3),
        ];
        prop_assert_eq!(u32::from_be_bytes(bytes), v);
        prop_assert_eq!(bus.read_long(addr), v);
        prop_assert_eq!(bus.read_word(addr), (v >> 16) as u16);
    }
}
