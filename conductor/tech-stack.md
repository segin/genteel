# Technology Stack: Genteel

## Core Language
*   **Rust (Edition 2021)**: Chosen for its performance, memory safety, and expressive type system, which are critical for high-performance hardware emulation.

## Emulation Core
*   **M68k & Z80 CPUs**: Custom implementations in pure Rust.
*   **Modular Bus Architecture**: Uses the \`MemoryInterface\` trait and \`SharedBus\` for component communication and memory mapping.

## Frontend and UI
*   **Winit**: Cross-platform window creation and input handling.
*   **Pixels**: Tiny hardware-accelerated pixel buffer for rendering the Genesis display.
*   **Egui**: Immediate mode GUI for the debugger interface and performance overlays.

## Audio
*   **CPAL / Rodio**: Cross-platform audio playback and stream management for the emulated sound chips (YM2612, PSG).

## Data and Serialization
*   **Serde / Serde_json**: For serializing the entire system state (save states) and handling configuration files.

## Testing and Quality Assurance
*   **Proptest**: Property-based testing for CPU instructions and flag behavior to ensure high compatibility.
*   **Cargo Test**: Standard unit and integration testing suite.
