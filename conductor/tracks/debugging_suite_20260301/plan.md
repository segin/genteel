# Implementation Plan: Comprehensive Developer Debugging Suite

## Phase 1: Infrastructure & UI Framework [checkpoint: f3a29f8]
- [x] Task: Implement the Multi-Window Framework in `gui.rs` using `egui::Window`. bd9acb7
- [x] Task: Add a "Debug" menu to the main GUI menu bar for toggling individual windows. 62dcb4f
- [x] Task: Implement window state persistence (saving/loading positions and visibility). 0438429
- [x] Task: Conductor - User Manual Verification 'Phase 1: Infrastructure & UI Framework' (Protocol in workflow.md) f3a29f8

## Phase 2: CPU & Execution Debugging [checkpoint: 63b4c8a]
- [x] Task: Implement Execution Control UI (Pause, Resume, Single Step). aad1c70
- [x] Task: Implement M68k Status Window (Registers, PC, SR, Flags). c36a695
- [x] Task: Implement Z80 Status Window (Registers, Flags, MEMPTR). afc0a13
- [x] Task: Implement Disassembly Viewer (Instruction stream around current PC). 221fa7f
- [x] Task: Conductor - User Manual Verification 'Phase 2: CPU & Execution Debugging' (Protocol in workflow.md) 63b4c8a

## Phase 3: VDP Debugging Tools [checkpoint: 722b975]
- [x] Task: Implement Palette Viewer (CRAM visualization with hex values). ba62ef8
- [x] Task: Implement Tile Viewer (VRAM pattern visualization). 43fff65
- [x] Task: Implement Sprite Viewer (Sprite attribute table and visual representation). 4b940f5
- [x] Task: Implement Scroll Plane Viewer (Visualization of Plane A, Plane B, and Window). 4b55486
- [x] Task: Implement VDP Memory Hex View (Raw editors for VRAM, CRAM, VSRAM). 41c4d6a
- [x] Task: Conductor - User Manual Verification 'Phase 3: VDP Debugging Tools' (Protocol in workflow.md) 722b975

## Phase 4: Audio & Memory Debugging
- [x] Task: Implement Memory Viewer (Hex editor for WRAM, Z80 RAM, and mapped ROM). d1a363b
- [ ] Task: Implement Sound Chip Visualizer (YM2612 FM parameters and PSG channel states).
- [ ] Task: Implement Audio Channel Waveforms (Real-time oscilloscope for each channel).
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Audio & Memory Debugging' (Protocol in workflow.md)

## Phase 5: System Status & Integration
- [ ] Task: Implement Controller Viewer (Input state and 3/6-button mode status).
- [ ] Task: Implement Dummy Expansion Status Window (Sega CD/32X placeholders).
- [ ] Task: Verify GDB Synchronization (UI updates when GDB pauses execution).
- [ ] Task: Conductor - User Manual Verification 'Phase 5: System Status & Integration' (Protocol in workflow.md)
