# AI Agent Integration and Development Process (`AGENTS.md`)

This document outlines the role and capabilities of AI agents in the `genteel` project, as well as the development process to be followed.

## Agent's Role and Purpose

The primary purpose of integrating AI agents with the `genteel` emulator is to enable automated testing, development, and exploration of Sega Mega Drive/Genesis software. By providing a machine-readable API to the emulator, we allow agents to:

-   **Develop the emulator**: Implement the features outlined in the project roadmap.
-   **Play games**: The agent can send controller inputs to play through games, discovering bugs and unexpected behaviors.
-   **Perform targeted testing**: The agent can be instructed to test specific parts of a game, such as a particular level or menu.
-   **Fuzzing**: The agent can generate random inputs to test the robustness of the emulator and the game.
-   **Analyze game states**: The agent can read the emulator's memory to analyze the game's internal state and make decisions based on it.

## Development Process and Agent Operating Principles

To ensure a smooth and transparent development process, the following principles must be followed:

1.  **Keep `ARCHITECTURE.md` in sync**: The `ARCHITECTURE.md` file is the source of truth for the project's design. Any changes to the architecture must be reflected in this document.
2.  **Commit on Task Completion**: At the end of each successfully completed task, all changes must be committed to the git repository with a clear and descriptive commit message.
3.  **Push on Commit**: Every `git commit` must be immediately followed by a `git push` to the remote repository.
4.  **Aggressive Testing**: All new features, especially CPU opcodes, must be accompanied by comprehensive unit and property-based tests (`proptest`). Tests must cover:
    -   Standard operation.
    -   Edge cases (e.g., overflow, zero, max/min values).
    -   Flag updates (C, V, Z, N, X for M68k; C, N, P/V, H, Z, S for Z80).
    -   A wide range of inputs using property-based testing to ensure robustness.

## Project Roadmap

This roadmap outlines the features to be implemented in the `genteel` emulator.

### Core Emulator Components
- [ ] **M68k CPU Core**: Implement the full instruction set of the Motorola 68000 CPU.
- [ ] **Z80 CPU Core**: Implement the full instruction set of the Zilog Z80 CPU.
- [ ] **Sega VDP (Video Display Processor)**: Implement the VDP to handle graphics rendering.
- [ ] **Yamaha YM2612 (FM Synthesizer)**: Implement the YM2612 for audio.
- [ ] **Texas Instruments SN76489 (PSG)**: Implement the SN76489 for additional audio.
- [ ] **Memory Bus**: Implement a flexible memory bus that maps all components to the correct address ranges.
- [ ] **I/O**: Implement support for game controllers.

### Testing
- [ ] **Unit Tests**: Add comprehensive unit tests for every component.
- [ ] **Property-Based Tests**: Use property-based testing (`proptest`) to test for a wide range of inputs.
- [ ] **Fuzz Testing**: Use fuzz testing (`cargo-fuzz`) to find crashes and vulnerabilities, especially in the CPU decoders.
- [ ] **Integration Tests**: Create integration tests using existing M68k and Z80 test suites.

### Debugging and Instrumentation
- [ ] **Assembler/Disassembler**: Implement an integrated assembler and disassembler for both M68k and Z80.
- [ ] **Hex Dumps**: Provide a function to view memory as a hex dump.
- [ ] **Screenshots**: Implement the ability to take screenshots of the VDP output.
- [ ] **TAS-like Input Queueing**: Create a system to queue up controller inputs for deterministic runs.
- [ ] **Debug Interface**: Implement a debug interface for running games (e.g., trapping on an instruction to print debug strings).
- [ ] **Execution Control**:
    - [ ] `step_instruction()`: Single-step M68k instructions.
    - [ ] `step_frame()`: Single-step video frames.
    - [ ] `run_for_frames(n)`: Run for a specified number of frames.

### API and Scripting
- [ ] **Rust API**: Expose all emulator features through a public Rust API.
- [ ] **Command-Line Interface**: Create a CLI to control the emulator (with no external dependencies).
- [ ] **Scripting Engine**: Implement a simple, text-based scripting engine for automating tasks (with no external dependencies).

---
*This document is managed by the AI agent and should be kept up-to-date with the latest project plan.*
