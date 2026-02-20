# Initial Concept
Genteel is an instrumentable Sega Mega Drive/Genesis emulator designed for automated testing by AI language models. It enables external agents to drive the emulator, inspect memory, and interact with the emulated system in a "native" M68k environment.

## Target Users
*   **AI Agents/LLMs**: Primary focus for automated testing and interaction.
*   **Homebrew Devs**: Developers who need high-quality Genesis emulation and debugging tools for their projects.
*   **Emulator Enthusiasts**: Users who want to play Genesis games with integrated debugging features.

## Core Goals
*   **Instrumentation**: Provide a robust API for external agents to inspect and modify memory, registers, and the emulated state.
*   **Automated Testing**: Enable seamless integration with CI/CD pipelines for testing Sega Genesis software.
*   **Accuracy**: Maintain a high level of hardware emulation fidelity to ensure test results are reliable.

## Key Features
*   **GDB Protocol Support**: Implement GDB Remote Serial Protocol (RSP) to allow standard debugging tools to connect and control the emulator.
*   **Integrated Debugger GUI**: A hybrid user interface using `egui` to display real-time VRAM, CRAM, and CPU state alongside the game output.
*   **Save States (Serde)**: Full serialization support for the system state, allowing for easy snapshotting and state recovery.

## Visual Style and UX
*   **Hybrid Layout**: A versatile interface that prioritizes the game view while offering toggleable, data-rich debug windows.
*   **Instrumented Feedback**: Real-time visual representation of internal system states (PC, V-Counter, etc.) to assist both human developers and AI observers.
