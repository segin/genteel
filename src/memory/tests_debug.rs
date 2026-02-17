use super::bus::Bus;
use crate::debugger::Debuggable;

#[test]
fn test_bus_debuggable() {
    let mut bus = Bus::new();

    // Modify Bus specific state
    bus.z80_bus_request = true;
    bus.z80_reset = false;
    bus.z80_bank_addr = 0x8000;

    // Modify VDP state (via Bus)
    // Bus.vdp is pub, so we can modify it directly.
    bus.vdp.registers[0] = 0xFF;
    // h_counter is private, but we can set it via set_h_counter if available, or just rely on registers.
    // Checking src/vdp/mod.rs again, set_h_counter is pub.
    bus.vdp.set_h_counter(0x1234);

    // Serialize state
    let state = bus.read_state();

    // Verify JSON structure
    assert!(state.get("z80_bus_request").is_some());
    assert!(state.get("z80_reset").is_some());
    assert!(state.get("z80_bank_addr").is_some());
    assert!(state.get("vdp").is_some());
    assert!(state.get("io").is_some());
    assert!(state.get("apu").is_some());

    // Verify values in JSON
    assert_eq!(state["z80_bus_request"], true);
    assert_eq!(state["z80_reset"], false);
    assert_eq!(state["z80_bank_addr"], 0x8000);
    assert_eq!(state["vdp"]["registers"][0], 0xFF);
    assert_eq!(state["vdp"]["h_counter"], 0x1234);

    // Create new Bus and restore state
    let mut new_bus = Bus::new();

    // Verify initial state is different
    // We can use read_state to check internal private fields like h_counter on the new bus too
    let new_bus_state_initial = new_bus.read_state();
    assert_ne!(new_bus.z80_bus_request, true);
    assert_ne!(new_bus.z80_reset, false);
    assert_ne!(new_bus.z80_bank_addr, 0x8000);
    assert_ne!(new_bus.vdp.registers[0], 0xFF);
    assert_ne!(new_bus_state_initial["vdp"]["h_counter"], 0x1234);

    // Restore state
    new_bus.write_state(&state);

    // Verify restored state
    // Again, use read_state to verify private fields if needed, or check pub fields directly
    assert_eq!(new_bus.z80_bus_request, true);
    assert_eq!(new_bus.z80_reset, false);
    assert_eq!(new_bus.z80_bank_addr, 0x8000);
    assert_eq!(new_bus.vdp.registers[0], 0xFF);

    // Check h_counter via read_state on the restored bus
    let restored_state = new_bus.read_state();
    assert_eq!(restored_state["vdp"]["h_counter"], 0x1234);

    // Note: IO and APU are not verified for restoration because their write_state is currently a no-op stub,
    // but their presence in JSON is verified above.
}
