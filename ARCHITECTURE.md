# `genteel` Architecture

This document outlines the architecture of the `genteel` emulator, providing a comprehensive understanding of its design and components.

## Project Identification

- **Project Name**: genteel
- **Version**: 0.1.0 (pre-alpha)
- **Description**: An instrumentable Sega Mega Drive/Genesis emulator designed for automated testing by AI language models.
- **Contact**: N/A
- **License**: (To be determined)

## 1. High-level Architecture

`genteel` is designed as a Rust application that provides a complete emulation of the Sega Mega Drive/Genesis console. It can be run as a standalone process and controlled by external tools through a debugging interface.

The high-level architecture is based on a modular design, where each major hardware component of the original console is implemented as a separate module. These components are coordinated by a central "emulator" or "system" object that manages the main emulation loop.

The main interaction flow is as follows:
1. An external tool starts the `genteel` application.
2. The external tool loads a game ROM into the emulator.
3. The external tool starts the main emulation loop.
4. In each iteration of the loop, the emulator:
    - Executes a number of CPU cycles for the main M68k CPU.
    - Executes a number of CPU cycles for the Z80 sound co-processor.
    - Updates the state of the VDP (Video Display Processor).
    - Updates the state of the sound chips.
    - Handles input from the external tool.
5. The external tool can pause the emulation at any time to inspect the state of the system, provide input, or get the video and audio output.

## 2. Core Components

The `genteel` emulator is composed of the following core components, each located in its own module:

- **`cpu`**: Implements the main processor, a Motorola 68000 (M68k). This component is responsible for fetching, decoding, and executing instructions from the game ROM and RAM.

- **`apu`**: The Audio Processing Unit. This module contains the implementation of the Zilog Z80 sound co-processor, the Yamaha YM2612 FM synthesizer, and the Texas Instruments SN76489 PSG.

- **`vdp`**: The Video Display Processor. This component is responsible for rendering the graphics. It manages video RAM (VRAM), sprites, backgrounds, and generates the video output.

- **`memory`**: This module implements the memory bus and memory mapping. It manages the different types of memory in the system, including ROM, RAM, and VRAM, and handles memory access from the different components.

- **`io`**: This module handles all input and output, including the game controllers.

- **`debugger`**: This module provides the functionality to debug the emulated system. It will implement the GDB Remote Serial Protocol (RSP) to allow external debuggers (like GDB) to connect to the emulator.

## 3. Data Stores

- **Game ROMs**: The primary data store is the game ROM file, which is loaded into the emulator's memory at the start of the emulation.
- **Save Games**: The emulator will eventually support saving and loading game states, which will be stored on the host filesystem.
- **Internal Memory**: The emulator manages several internal memory regions, including:
    - **Work RAM (WRAM)**: 64KB of general-purpose RAM for the M68k.
    - **Video RAM (VRAM)**: 64KB of RAM for the VDP to store graphics data.
    - **Sound RAM**: 8KB of RAM for the Z80.

## 4. External System Integrations

The "instrumentable" nature of `genteel` is achieved through two main interfaces:

- **AI Agent API**: A high-level API for AI agents to control the emulator. See `AGENTS.md` for more details on the planned integration of AI agents. The planned API will include functions to:
    - `load_rom(rom_data: &[u8])`: Load a game ROM.
    - `run_frame()`: Execute the emulation for a single frame.
    - `set_controller_state(port: u8, state: ControllerState)`: Set the state of a game controller.
    - `read_memory(address: u32) -> u8`: Read a byte from a specific memory address.
    - `write_memory(address: u32, value: u8)`: Write a byte to a specific memory address.
    - `get_screen_buffer() -> &[u8]`: Get the current video frame as a raw image buffer.

- **Debugger Interface**: A low-level interface for debuggers. This is described in more detail in the "Debugging and Instrumentation" section.

To facilitate these integrations, each major component of the emulator will implement the `Debuggable` trait, which provides a standardized way to read and write the component's state.

## 5. Deployment and Infrastructure

`genteel` is a standalone application that can be run on Linux, macOS, and Windows. It will be built and tested using `cargo`.

## 6. Security Considerations

As an emulator, `genteel` runs code from untrusted sources (game ROMs). Care must be taken to ensure that the emulator is robust against malformed or malicious ROMs. This includes:
- Bounds checking for all memory accesses.
- Correct handling of invalid instructions.

The debugger interface will be exposed on a local network socket. Access to this interface should be restricted to trusted users.

## 7. Development & Testing Environment

- **Language**: Rust
- **Build Tool**: `cargo`
- **Testing**: `cargo test`. We will aim to have a comprehensive test suite, including:
    - Unit tests for individual components.
    - Integration tests that run small test ROMs and check the state of the emulator.
    - Comparison with other emulators to ensure accuracy.

## 8. Future Considerations & Roadmap

The development of `genteel` will follow this general roadmap:

1.  **M68k CPU Core**: Implement a functional M68k CPU core that can execute basic instructions.
2.  **Memory Bus**: Implement the memory bus and ROM loading.
3.  **VDP Implementation**: Implement the VDP to get basic graphics output.
4.  **APU Implementation**: Implement the Z80 and sound chips for audio.
5.  **Instrumentation API**: Implement the full instrumentation API for external control.
6.  **Debugger Implementation**: Implement the GDB Remote Serial Protocol in the `debugger` module.
7.  **Accuracy Improvements**: Continuously improve the accuracy of the emulation by running test suites and comparing with real hardware.

## 9. Glossary

- **M68k**: Motorola 68000, the main CPU of the Sega Mega Drive/Genesis.
- **VDP**: Video Display Processor, the custom graphics chip.
- **Z80**: Zilog Z80, an 8-bit CPU used as a sound co-processor.
- **YM2612**: A six-voice FM synthesis sound chip.
- **SN76489**: A programmable sound generator (PSG) chip.
- **ROM**: Read-Only Memory, where the game's code and data are stored.
- **GDB RSP**: The GDB Remote Serial Protocol, a protocol for remote debugging.

## 10. Debugging and Instrumentation

To allow for deep inspection and control of the emulated system, `genteel` will implement a debugging interface compatible with the GDB Remote Serial Protocol (RSP). This will allow developers to use standard debuggers like GDB to debug the M68k code running inside the emulator.

The debugging features will include:
- **Execution control**: Stepping through instructions, setting breakpoints, and continuing execution.
- **Register access**: Reading and writing the M68k CPU registers.
- **Memory access**: Reading and writing to any part of the emulated memory map.

This functionality will be provided by the `debugger` module. Each component of the emulator that holds state (like the CPU and memory) will implement the `Debuggable` trait, which allows the `debugger` module to access and modify its internal state.
