use crate::vdp::Vdp;

#[test]
fn test_control_state_machine() {
    let mut vdp = Vdp::new();

    // 1. Initial state
    assert!(
        !vdp.is_control_pending(),
        "Initial control_pending should be false"
    );

    // 2. First word of a command (0x4000 = VRAM Write)
    vdp.write_control(0x4000);
    assert!(
        vdp.is_control_pending(),
        "Control pending should be true after first word"
    );
    // CD1-0 = 01 (VRAM Write)
    assert_eq!(
        vdp.command.code & 0x03,
        0x01,
        "Control code bits 1-0 should be 01"
    );
    // Address part 1: A13-0 are bits 13-0 of value. 0x4000 is 0100 0000 0000 0000.
    // control_address = (value & 0x3FFF) = 0.
    assert_eq!(vdp.command.address, 0x0000, "Control address should be 0");

    // 3. Second word completes command (addr 0x4000)
    vdp.reset();
    vdp.write_control(0x4000);
    vdp.write_control(0x0001); // Bit 0 -> A14
    assert!(
        !vdp.is_control_pending(),
        "Control pending should be false after second word"
    );
    assert_eq!(
        vdp.command.address, 0x4000,
        "Control address should be 0x4000"
    );

    // 4. Test "register write interrupted"
    vdp.reset();
    vdp.write_control(0x4000);
    assert!(vdp.is_control_pending());

    // Write what looks like a register write (0x8144 - enable display)
    // If pending is true, this should be treated as 2nd word of command!
    vdp.write_control(0x8144);

    assert!(
        !vdp.is_control_pending(),
        "Control pending should be false after second word (even if looks like reg write)"
    );
    // Register 1 should NOT be updated to 0x44 (enable display) if it was treated as data
    // Initial reg 1 is 0.
    assert_eq!(vdp.registers[1], 0x00, "Register 1 should not be updated");

    // Verify address/code updated based on this 2nd word.
    // 0x8144 -> 0x11
    assert_eq!(
        vdp.command.code, 0x11,
        "Control code should be 0x11 (0x01 | 0x10)"
    );

    // 5. Test data write clears pending
    vdp.reset();
    vdp.write_control(0x4000);
    assert!(vdp.is_control_pending());
    vdp.write_data(0x1234);
    assert!(
        !vdp.is_control_pending(),
        "Write data should clear control pending"
    );

    // 6. Test data read clears pending
    vdp.reset();
    vdp.write_control(0x4000);
    assert!(vdp.is_control_pending());
    vdp.read_data();
    assert!(
        !vdp.is_control_pending(),
        "Read data should clear control pending"
    );

    // 7. Test status read clears pending
    vdp.reset();
    vdp.write_control(0x4000);
    assert!(vdp.is_control_pending());
    // Reading the status register clears the pending flag (confirmed hardware behavior).
    vdp.read_status();
    assert!(
        !vdp.is_control_pending(),
        "Read status must clear control pending"
    );

    // Verify next write starts a new command
    vdp.write_control(0x4000);
    assert!(
        vdp.is_control_pending(),
        "Next write should be treated as first word of new command"
    );
}

#[test]
fn test_write_state_reconstructs_cram_cache() {
    use crate::debugger::Debuggable;

    let mut source_vdp = Vdp::new();

    // Set color 1 to Red manually in CRAM
    // 0x000E -> B=0, G=0, R=7
    // CRAM is little-endian byte array in Vdp struct logic (index 2 is low byte)
    source_vdp.cram[2] = 0x0E;
    source_vdp.cram[3] = 0x00;

    // Serialize
    let state = source_vdp.read_state();

    // Create new Vdp
    let mut dest_vdp = Vdp::new();

    // Write state (should trigger reconstruct_cram_cache)
    dest_vdp.write_state(&state);

    // Verify CRAM cache in dest_vdp
    // Genesis 0x000E -> RGB565 0xF800 (Red)
    assert_eq!(
        dest_vdp.cram_cache[1], 0xF800,
        "CRAM cache should be reconstructed from state"
    );
}

#[test]
fn test_mode1_getter_behavior() {
    let mut vdp = Vdp::new();

    // Mode 1 getter should return the value of register 0
    // Command 0x8014 writes 0x14 to register 0 (0x8000 | (0 << 8) | 0x14)
    vdp.write_control(0x8014);

    assert_eq!(
        vdp.mode1(),
        0x14,
        "mode1() did not return the expected value from register 0"
    );
}

/// Setting M3 (register 0 bit 1) through the control port latches the HV
/// counter at once; reads stay frozen while M3 stays set and follow the beam
/// again once it is cleared (GPGX `vdp_reg_w` case 0).
#[test]
fn m3_register_write_latches_hv_counter() {
    use crate::debugger::Debuggable;
    let mut vdp = Vdp::new();
    vdp.v_counter = 40;
    vdp.mclk_line_clocks = 1200;
    let at_latch = vdp.read_hv_counter();
    vdp.write_control(0x8002);
    vdp.tick(3420 * 3 + 500, |_| 0);
    assert_eq!(vdp.read_hv_counter(), at_latch, "frozen while M3 is set");

    // Rewriting register 0 with M3 still set does not relatch.
    vdp.write_control(0x8012);
    assert_eq!(vdp.read_hv_counter(), at_latch);

    // The latch survives a save state.
    let mut restored = Vdp::new();
    restored.write_state(&vdp.read_state());
    assert_eq!(restored.read_hv_counter(), at_latch);

    vdp.write_control(0x8000);
    assert_ne!(vdp.read_hv_counter(), at_latch, "live once M3 is cleared");
    vdp.write_control(0x8002);
    assert_eq!(Some(vdp.read_hv_counter()), vdp.hv_latched, "relatched");
    assert_ne!(vdp.read_hv_counter(), at_latch);
}

/// A save state taken between a line interrupt and its acknowledge still
/// has the interrupt pending after loading.
#[test]
fn save_state_keeps_pending_hint() {
    use crate::debugger::Debuggable;
    let mut vdp = Vdp::new();
    vdp.registers[crate::vdp::REG_MODE1] = crate::vdp::MODE1_HINT_ENABLE;
    vdp.registers[crate::vdp::REG_H_INT_COUNTER] = 0;
    vdp.v_counter = 20;
    vdp.mclk_line_clocks = 3419;
    vdp.tick(1, |_| 0);
    assert!(vdp.hint_pending());

    let mut restored = Vdp::new();
    restored.write_state(&vdp.read_state());
    assert!(restored.hint_pending(), "HINT still pending after load");
    restored.acknowledge_hint();
    assert!(!restored.hint_pending());
}

/// The second word of a control-port long write whose first word starts a
/// 68k DMA is held until the DMA completes, so it cannot change the running
/// DMA (GPGX `vdp_68k_ctrl_w` `cached_write`).
#[test]
fn control_word_during_68k_dma_waits_for_its_end() {
    use crate::vdp::{DmaOps, REG_DMA_LEN_HI, REG_DMA_LEN_LO, REG_DMA_SRC_HI, REG_MODE2};
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE2] = crate::vdp::MODE2_DMA_ENABLE;
    vdp.registers[crate::vdp::REG_AUTO_INC] = 2;
    vdp.registers[REG_DMA_SRC_HI] = 0x7F;
    vdp.registers[REG_DMA_LEN_LO] = 4;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080); // starts the 68k DMA
    assert!(vdp.command.bus_dma_active());
    vdp.write_control(0x9402); // register 20 = 2: held back
    assert_eq!(vdp.registers[REG_DMA_LEN_HI], 0);

    let mut words = 0;
    while vdp.command.dma_pending {
        vdp.step_dma(&mut |_| {
            words += 1;
            0x1234
        });
    }
    assert_eq!(words, 4, "the DMA ran with its original length");
    assert_eq!(vdp.registers[REG_DMA_LEN_HI], 2, "written after the DMA");
    assert_eq!(vdp.dma_length(), 0x200);
}
