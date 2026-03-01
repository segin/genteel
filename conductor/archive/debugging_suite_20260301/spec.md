# Specification: Comprehensive Developer Debugging Suite

## Overview
This track involves the implementation of a comprehensive, multi-window debugging suite for the `genteel` emulator. The goal is to provide software developers with a "one-stop shop" for debugging Sega Genesis/Mega Drive games and utility software. The suite will leverage `egui` for an integrated, multi-window interface that doesn't obstruct the main game view.

## Functional Requirements
1.  **Integrated Multi-Window UI**:
    -   Use `egui` to implement movable, toggleable, and dockable internal windows.
    -   Support multiple simultaneous viewers without obscuring the primary game output.
2.  **VDP Debugging Tools**:
    -   **Palette Viewer**: Display all CRAM entries with hex values.
    -   **Tile Viewer**: Visualize tiles from VRAM (patterns).
    -   **Scroll Plane Viewer**: Visualize Plane A, Plane B, and Window planes.
    -   **Sprite Viewer**: List active sprites, their attributes, and visual representation.
    -   **VDP Memory Hex View**: Raw hex editors for VRAM, CRAM, and VSRAM.
3.  **CPU Debugging Tools**:
    -   **Execution Control**: Pause, resume, and single-step execution.
    -   **M68k Status**: Real-time display of registers (D0-D7, A0-A7), PC, SR, and flags.
    -   **Z80 Status**: Real-time display of registers (A, F, BC, DE, HL, IX, IY, SP, PC), I, R, and flags.
    -   **Disassembly View**: Show the instruction stream around the current PC for both CPUs.
4.  **Audio Debugging Tools (APU)**:
    -   **Sound Chip Visualizer**:
        -   **FM Parameters**: Real-time display of YM2612 operators, envelopes, and algorithms.
        -   **PSG State**: Visualization of the 4 PSG channels (tone/noise).
        -   **Channel Waveforms**: Oscilloscope-style waveforms for each output channel.
5.  **Memory Debugging**:
    -   **Memory Viewer**: Hex editor for WRAM, Z80 RAM, and mapped ROM.
6.  **System & Peripheral Status**:
    -   **Controller Viewer**: Show current input state and controller mode (3-button vs 6-button).
    -   **Expansion Status**: Placeholder/Status for SEGA CD and 32X expansions.
7.  **GDB Synchronization**:
    -   Automatically update the UI state when the emulator hits a breakpoint or is paused via GDB.

## Non-Functional Requirements
-   **Performance**: The debug windows should not significantly degrade emulation FPS when open.
-   **UX**: Windows should remember their positions and visibility states (serialized via `egui`).

## Acceptance Criteria
-   All requested viewers are accessible via a "Debug" menu in the GUI.
-   The user can pause/step the emulator and see real-time updates in all open debug windows.
-   Multiple windows can be open at once without overlapping the game view if arranged by the user.
-   FM/PSG visualizations update in real-time during audio playback.

## Out of Scope
-   External OS-level windows.
-   Full SEGA CD emulation (only status/placeholders for now).
-   Advanced trace logging (to be handled in a separate track).
