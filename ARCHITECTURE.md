# Architecture Overview

This document serves as a critical, living template designed to equip agents with a rapid and comprehensive understanding of the codebase's architecture, enabling efficient navigation and effective contribution from day one. Update this document as the codebase evolves.

## 1. Project Structure

This section provides a high-level overview of the project's directory and file structure, categorised by architectural layer or major functional area. It is essential for quickly navigating the codebase, locating relevant files, and understanding the overall organization and separation of concerns.

```text
genteel/
├── src/
│   ├── main.rs           # Application entry point
│   ├── lib.rs            # Library exports
│   ├── cpu/              # M68k CPU implementation
│   │   └── mod.rs
│   ├── apu/              # Audio Processing Unit (Z80, YM2612, SN76489)
│   │   └── mod.rs
│   ├── vdp/              # Video Display Processor
│   │   └── mod.rs
│   ├── memory/           # Memory bus and mapping
│   │   └── mod.rs
│   ├── io/               # Input/Output (controllers)
│   │   └── mod.rs
│   └── debugger/         # GDB RSP debugging interface
│       └── mod.rs
├── docs/                 # Additional documentation
├── scripts/              # Automation and auditing scripts
├── Cargo.toml            # Rust project manifest
├── README.md             # Project overview for humans
├── AGENTS.md             # AI agent operational context
└── ARCHITECTURE.md       # This document
```

## 2. High-Level System Diagram

Provide a simple block diagram or a clear text-based description of the major components and their interactions.

```text
[External Tool (e.g., Agent, GDB)] 
       ^
       |
       v
[Genteel Emulator (main.rs / lib.rs)] <--> [SharedBus / MemoryInterface]
                                                  |
                                                  +--> [CPU (M68k)]
                                                  |
                                                  +--> [APU (Z80, YM2612, PSG)]
                                                  |
                                                  +--> [VDP (Graphics)]
                                                  |
                                                  +--> [I/O (Controllers)]
                                                  |
                                                  +--> [Memory (ROM, WRAM, VRAM, SRAM)]
```

## 3. Core Components

### 3.1. Emulator Core
Name: Emulator / System Loop
Description: Coordinates the main emulation loop, steps the M68k CPU, steps the Z80 co-processor, updates VDP state, checks for H/V interrupts, and manages bus contention.
Technologies: Rust

### 3.2. M68k CPU
Name: CPU (`src/cpu/`)
Description: Implements the main processor, a Motorola 68000 (M68k). Responsible for fetching, decoding, and executing instructions from the game ROM and RAM.
Technologies: Rust

### 3.3. Audio Processing Unit (APU)
Name: APU (`src/apu/`)
Description: Contains the implementation of the Zilog Z80 sound co-processor, the Yamaha YM2612 FM synthesizer, and the Texas Instruments SN76489 PSG. The Z80 implementation handles architectural nuances like MEMPTR (WZ Register), R Register wrapping, and EI interrupt shadowing.
Technologies: Rust

### 3.4. Video Display Processor (VDP)
Name: VDP (`src/vdp/`)
Description: Responsible for rendering the graphics. It manages video RAM (VRAM), sprites, backgrounds, and generates the video output.
Technologies: Rust

### 3.5. Memory & Bus
Name: Memory (`src/memory/`)
Description: Implements the memory bus and mapping via the `MemoryInterface` trait. The `SharedBus` wrapper allows components to share the `Bus` state. Supports full state serialization/deserialization via `serde`.
Technologies: Rust (Trait Objects, Interior Mutability, Serde)

### 3.6. I/O and Debugger
Name: I/O (`src/io/`) & Debugger (`src/debugger/`)
Description: Handles all input and output (game controllers), and provides a GDB Remote Serial Protocol (RSP) interface to allow external debuggers to connect to the emulator.
Technologies: Rust, GDB RSP

## 4. Data Stores

### 4.1. Game ROMs
Name: Primary Game ROM
Type: File (binary/zip)
Purpose: Read-Only Memory where the game's code and data are stored. Loaded into the emulator's memory at the start.

### 4.2. Internal Memory
Name: System RAM
Type: In-memory buffers
Purpose: The emulator manages several internal memory regions: Work RAM (64KB for M68k), Video RAM (64KB for VDP), and Sound RAM (8KB for Z80).

### 4.3. Save States
Name: Save Games
Type: File (binary/JSON)
Purpose: Future support for saving and loading game states, utilizing `serde` serialization to store component states.

## 5. External Integrations / APIs

Service Name 1: AI Agent API
Purpose: High-level API for AI agents to control the emulator (load ROMs, run frames, set controller state, read/write memory).
Integration Method: Rust API / Command-line arguments (`--script`, `--headless`)

Service Name 2: GDB Interface
Purpose: Allows external standard debuggers (like GDB) to connect and debug M68k code running inside the emulator.
Integration Method: Local network socket (GDB RSP)

## 6. Deployment & Infrastructure

Cloud Provider: Local / Standalone Application (Linux, macOS, Windows)
Key Services Used: Native OS execution
CI/CD Pipeline: To be configured (Supports headless validation for CI)
Monitoring & Logging: `log` and `env_logger` crates for structured output (`RUST_LOG=debug`)

## 7. Security Considerations

Authentication: N/A
Authorization: N/A
Data Encryption: N/A
Key Security Tools/Practices:
- Bounds-check all memory accesses (emulator runs untrusted ROM code).
- Handle invalid/malformed instructions gracefully.
- The debugger interface uses local network sockets - restrict access appropriately.
- Run the audit tool (`python3 scripts/audit_tool.py`) periodically to detect secrets and unsafe patterns.

## 8. Development & Testing Environment

Local Setup Instructions: `cargo build` (debug) or `cargo build --release`
Testing Frameworks: `cargo test` (unit/integration), `proptest` (property-based tests), `cargo-fuzz` (fuzzing)
Code Quality Tools: `cargo clippy`, `cargo fmt`, `make audit` (custom audit script)

## 9. Future Considerations / Roadmap

- **M68k Implementation**: Completed full instruction set and addressing modes; fixed bugs in ADDX/SUBX/EXG.
- **APU Implementation**: Initial Yamaha YM2612 FM and SN76489 PSG support implemented; fixed test regressions.
- **AI Agent API**: Expanded scripting engine with memory/register manipulation commands (READ/WRITE/ASSERT).
- **Controller Support**: 3-button and 6-button controller support implemented.
- **GDB Support**: Basic RSP support with breakpoints and inspection implemented.
- **Accuracy Improvements**: Moved VBlank/HBlank/LineCounter management into VDP `tick` for better cycle accuracy.
- **32X Expansion**: Future goal (dual SH2 cores, Master/Slave sync, 32X VDP).

## 10. Project Identification

Project Name: genteel
Repository URL: https://github.com/segin/genteel
Primary Contact/Team: N/A
Date of Last Update: 2026-02-25 (Major Update: Core Fixes & Scripting Expansion)

## 11. Glossary / Acronyms

M68k: Motorola 68000, the main CPU of the Sega Mega Drive/Genesis
VDP: Video Display Processor, the custom graphics chip
Z80: Zilog Z80, an 8-bit CPU used as a sound co-processor
YM2612: A six-voice FM synthesis sound chip
SN76489: A programmable sound generator (PSG) chip
ROM: Read-Only Memory, where the game's code and data are stored
GDB RSP: The GDB Remote Serial Protocol, a protocol for remote debugging
VRAM: Video RAM, memory used by the VDP for graphics
WRAM: Work RAM, general-purpose memory for the M68k
