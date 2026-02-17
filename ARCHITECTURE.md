# `genteel` Architecture

This document outlines the architecture of the `genteel` emulator, providing a comprehensive understanding of its design and components.

## Project Identification

- **Project Name**: genteel
- **Version**: 0.1.0 (pre-alpha)
- **Description**: An instrumentable Sega Mega Drive/Genesis emulator designed for automated testing by AI language models.
- **Repository**: (To be added)
- **Contact**: N/A
- **License**: (To be determined)
- **Date of Last Update**: 2026-01-07

## 1. High-level Architecture

`genteel` is designed as a Rust application that provides a complete emulation of the Sega Mega Drive/Genesis console. It can be run as a standalone process and controlled by external tools through a debugging interface.

The high-level architecture is based on a modular design, where each major hardware component of the original console is implemented as a separate module. These components are coordinated by a central "emulator" or "system" object that manages the main emulation loop.

The main interaction flow is as follows:
1. An external tool starts the `genteel` application.
2. The external tool loads a game ROM into the emulator.
3. The external tool starts the main emulation loop.
4. In each iteration of the loop, the emulator:
    - Steps the M68k CPU (linked to the main Genesis bus).
    - Steps the Z80 co-processor (with its dedicated Sound RAM).
    - Updates VDP state and checks for H/V interrupts.
5. Multi-component bus contention is managed through a `SharedBus` wrapper and the `MemoryInterface` trait.
5. The external tool can pause the emulation at any time to inspect the state of the system, provide input, or get the video and audio output.

## 2. Project Structure

```
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
├── Cargo.toml            # Rust project manifest
├── README.md             # Project overview for humans
├── AGENTS.md             # AI agent operational context
└── ARCHITECTURE.md       # This document
```

## 3. Core Components

The `genteel` emulator is composed of the following core components, each located in its own module:

### 3.1. CPU (`src/cpu/`)
**Description**: Implements the main processor, a Motorola 68000 (M68k). This component is responsible for fetching, decoding, and executing instructions from the game ROM and RAM.

**Technologies**: Rust

**Key Interfaces**:
- Instruction fetch and decode
- Register access
- Exception handling

### 3.2. APU (`src/apu/`)
**Description**: The Audio Processing Unit. This module contains the implementation of the Zilog Z80 sound co-processor, the Yamaha YM2612 FM synthesizer, and the Texas Instruments SN76489 PSG.

**Technologies**: Rust

### 3.3. VDP (`src/vdp/`)
**Description**: The Video Display Processor. This component is responsible for rendering the graphics. It manages video RAM (VRAM), sprites, backgrounds, and generates the video output.

**Technologies**: Rust

### 3.4. Memory (`src/memory/`)
**Description**: Implements the memory bus and memory mapping via the `MemoryInterface` trait. The M68k CPU uses a trait object (`Box<dyn MemoryInterface>`) to access the `Bus`, which routes requests to ROM, WRAM, VDP, and I/O.

The `SharedBus` wrapper allows multiple components to share the same `Bus` state via `Rc<RefCell<Bus>>`.

**Technologies**: Rust (Trait Objects, Interior Mutability)

### 3.5. I/O (`src/io/`)
**Description**: This module handles all input and output, including the game controllers.

**Technologies**: Rust

### 3.6. Debugger (`src/debugger/`)
**Description**: This module provides the functionality to debug the emulated system. It will implement the GDB Remote Serial Protocol (RSP) to allow external debuggers (like GDB) to connect to the emulator.

**Technologies**: Rust, GDB RSP

## 4. Data Stores

### 4.1. Game ROMs
**Type**: File (binary)
**Purpose**: The primary data store is the game ROM file, which is loaded into the emulator's memory at the start of the emulation.

### 4.2. Internal Memory
**Type**: In-memory buffers
**Purpose**: The emulator manages several internal memory regions:

| Memory Type | Size | Purpose |
|-------------|------|---------|
| Work RAM (WRAM) | 64KB | General-purpose RAM for M68k |
| Video RAM (VRAM) | 64KB | Graphics data for VDP |
| Sound RAM | 8KB | RAM for Z80 co-processor |

### 4.3. Save Games
**Type**: File (binary)
**Purpose**: The emulator will eventually support saving and loading game states, which will be stored on the host filesystem.

## 5. External System Integrations

The "instrumentable" nature of `genteel` is achieved through two main interfaces:

### 5.1. AI Agent API
A high-level API for AI agents to control the emulator. See `AGENTS.md` for more details on agent integration. The planned API will include:

```rust
fn load_rom(rom_data: &[u8]);
fn run_frame();
fn set_controller_state(port: u8, state: ControllerState);
fn read_memory(address: u32) -> u8;
fn write_memory(address: u32, value: u8);
fn get_screen_buffer() -> &[u8];
```

### 5.2. Debugger Interface
A low-level interface for debuggers implementing the GDB Remote Serial Protocol (RSP).

To facilitate these integrations, each major component implements the `Debuggable` trait for standardized state access.

## 6. Deployment and Infrastructure

- **Platform**: Linux, macOS, Windows
- **Build Tool**: `cargo`
- **CI/CD**: (To be configured)

## 7. Security Considerations

As an emulator, `genteel` runs code from untrusted sources (game ROMs). Care must be taken to ensure robustness:

- **Memory Safety**: Bounds checking for all memory accesses
- **Instruction Handling**: Correct handling of invalid instructions
- **File Access**: ROM loading is restricted to whitelisted directories (or the directory of the initially loaded ROM) to prevent path traversal
- **Network Security**: The debugger interface on local network sockets should be restricted to trusted users

## 8. Development & Testing Environment

- **Language**: Rust
- **Build Tool**: `cargo`
- **Testing Framework**: `cargo test`
- **Property Testing**: `proptest` crate
- **Fuzz Testing**: `cargo-fuzz`
- **Code Quality**: `cargo clippy`, `cargo fmt`

### Test Coverage Goals:
- Unit tests for individual components
- Integration tests with small test ROMs
- Comparison with other emulators for accuracy verification

## 9. Future Considerations & Roadmap

### Phase 1: M68k CPU Core
- [ ] Implement full instruction set of the Motorola 68000 CPU
- [ ] All addressing modes
- [ ] Exception handling

### Phase 2: Memory Bus
- [x] Implement the memory bus and ROM loading
- [x] Full Genesis memory map
- [x] Memory-mapped I/O

### Phase 3: VDP Implementation
- [x] Basic VDP register handling
- [x] Tile/pattern rendering
- [x] Sprite rendering
- [x] Background layers (A, B)

### Phase 4: APU & System Integration
- [x] Z80 CPU core
- [x] Unified Genesis Memory Bus implementation
- [x] M68k/Z80/Bus Integration via SharedBus
- [ ] Yamaha YM2612 FM synthesizer
- [ ] Texas Instruments SN76489 PSG

### Phase 5: Instrumentation API
- [ ] Rust API for external control
- [ ] Command-line interface
- [ ] Simple scripting engine

### Phase 6: I/O and Controllers
- [ ] 3-button controller support
- [ ] 6-button controller support

### Phase 7: Debugger Implementation
- [ ] GDB Remote Serial Protocol support
- [ ] Breakpoints
- [ ] Register/memory inspection

### Phase 8: Accuracy Improvements
- [ ] Run existing test suites
- [ ] Compare with real hardware
- [ ] Cycle-accurate timing (stretch goal)
- [x] Z80 Torture Phase (MEMPTR, IM2 Vectors, EI Latency)

### Phase 9: 32X Expansion
- [ ] Implement dual Hitachi SH7604 (SH2) CPU cores
- [ ] Implement Master/Slave synchronization logic
- [ ] 32X VDP (Shim) and Framebuffer rendering
- [ ] Shared SDRAM and Communication Bridge


### Testing Roadmap
- [ ] Unit tests for every component
- [ ] Property-based tests with `proptest`
- [ ] Fuzz testing with `cargo-fuzz`
- [ ] Integration tests with M68k/Z80 test suites

### Debugging & Instrumentation Features
- [ ] Assembler/Disassembler for M68k and Z80
- [ ] Memory hex dump viewer
- [ ] Screenshot capture
- [ ] TAS-like input queueing
- [ ] Execution control (`step_instruction()`, `step_frame()`, `run_for_frames(n)`)

## 10. Glossary

| Term | Definition |
|------|------------|
| **M68k** | Motorola 68000, the main CPU of the Sega Mega Drive/Genesis |
| **VDP** | Video Display Processor, the custom graphics chip |
| **Z80** | Zilog Z80, an 8-bit CPU used as a sound co-processor |
| **YM2612** | A six-voice FM synthesis sound chip |
| **SN76489** | A programmable sound generator (PSG) chip |
| **ROM** | Read-Only Memory, where the game's code and data are stored |
| **GDB RSP** | The GDB Remote Serial Protocol, a protocol for remote debugging |
| **VRAM** | Video RAM, memory used by the VDP for graphics |
| **WRAM** | Work RAM, general-purpose memory for the M68k |

## 11. Debugging and Instrumentation

To allow for deep inspection and control of the emulated system, `genteel` will implement a debugging interface compatible with the GDB Remote Serial Protocol (RSP). This will allow developers to use standard debuggers like GDB to debug the M68k code running inside the emulator.

The debugging features will include:
- **Execution control**: Stepping through instructions, setting breakpoints, and continuing execution.
- **Register access**: Reading and writing the M68k CPU registers.
- **Memory access**: Reading and writing to any part of the emulated memory map.

This functionality will be provided by the `debugger` module. Each component of the emulator that holds state (like the CPU and memory) will implement the `Debuggable` trait, which allows the `debugger` module to access and modify its internal state.
## 12. Z80 Architectural Nuances

To achieve high compatibility with Sega Genesis software, the Z80 implementation must adhere to several undocumented behaviors:

### 12.1. MEMPTR (WZ Register)
An internal 16-bit register used for temporary storage during 16-bit operations. Its state "leaks" into the X (bit 3) and Y (bit 5) flags during `BIT n, (HL)` instructions.

### 12.2. R Register (Refresh)
The lower 7 bits of the R register increment on every instruction fetch (including prefixes). Bit 7 is preserved and can only be modified via `LD R, A`.

### 12.3. Interrupt Latency
The `EI` instruction (Enable Interrupts) disables maskable interrupts for the instruction immediately following it. This "interrupt shadow" is critical for safe stack manipulation.

### 12.4. IM 2 Interrupts
In Mode 2, the CPU fetches an 8-bit vector from the data bus, combines it with the `I` register to form a 16-bit address, and jumps to the address stored at that location.
