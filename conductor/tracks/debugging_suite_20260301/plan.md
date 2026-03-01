# Implementation Plan: Comprehensive Developer Debugging Suite

## Phase 1: Infrastructure & UI Framework
- [x] Task: Implement the Multi-Window Framework in `gui.rs` using `egui::Window`. bd9acb7
- [ ] Task: Add a "Debug" menu to the main GUI menu bar for toggling individual windows.
- [ ] Task: Implement window state persistence (saving/loading positions and visibility).
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Infrastructure & UI Framework' (Protocol in workflow.md)

## Phase 2: CPU & Execution Debugging
- [ ] Task: Implement Execution Control UI (Pause, Resume, Single Step).
- [ ] Task: Implement M68k Status Window (Registers, PC, SR, Flags).
- [ ] Task: Implement Z80 Status Window (Registers, Flags, MEMPTR).
- [ ] Task: Implement Disassembly Viewer (Instruction stream around current PC).
- [ ] Task: Conductor - User Manual Verification 'Phase 2: CPU & Execution Debugging' (Protocol in workflow.md)

## Phase 3: VDP Debugging Tools
- [ ] Task: Implement Palette Viewer (CRAM visualization with hex values).
- [ ] Task: Implement Tile Viewer (VRAM pattern visualization).
- [ ] Task: Implement Sprite Viewer (Sprite attribute table and visual representation).
- [ ] Task: Implement Scroll Plane Viewer (Visualization of Plane A, Plane B, and Window).
- [ ] Task: Implement VDP Memory Hex View (Raw editors for VRAM, CRAM, VSRAM).
- [ ] Task: Conductor - User Manual Verification 'Phase 3: VDP Debugging Tools' (Protocol in workflow.md)

## Phase 4: Audio & Memory Debugging
- [ ] Task: Implement Memory Viewer (Hex editor for WRAM, Z80 RAM, and mapped ROM).
- [ ] Task: Implement Sound Chip Visualizer (YM2612 FM parameters and PSG channel states).
- [ ] Task: Implement Audio Channel Waveforms (Real-time oscilloscope for each channel).
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Audio & Memory Debugging' (Protocol in workflow.md)

## Phase 5: System Status & Integration
- [ ] Task: Implement Controller Viewer (Input state and 3/6-button mode status).
- [ ] Task: Implement Dummy Expansion Status Window (Sega CD/32X placeholders).
- [ ] Task: Verify GDB Synchronization (UI updates when GDB pauses execution).
- [ ] Task: Conductor - User Manual Verification 'Phase 5: System Status & Integration' (Protocol in workflow.md)
