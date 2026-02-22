# Genteel

Genteel is an instrumentable, highly-verified Sega Mega Drive/Genesis emulator built in Rust. It is uniquely designed to be equally accessible to **human developers**, **CI/CD pipelines**, and **AI agents**.

The project's primary mission is to provide an accurate, transparent, and scriptable emulation environment for Genesis software development and automated validation.

## 🛠 Debugging for Humans

Genteel provides multiple layers of transparency into the emulated system state:

*   **Standard GDB Support:** A built-in GDB stub implementing the Remote Serial Protocol (RSP). You can connect standard tools like `gdb` or `gdbgui` to debug M68k code running inside the emulator, set breakpoints, and inspect registers.
*   **Performance & Debug Overlay:** When running with the GUI, a real-time overlay provides critical stats:
    *   Frontend and internal FPS tracking.
    *   Current Program Counter (PC) for both M68k and Z80.
    *   VDP (Video) status, display state, and background color index.
    *   Direct CRAM inspection.
*   **Structured Logging:** Leverages the `log` crate for detailed execution traces. Use `RUST_LOG=debug` to see cycle-by-cycle component interactions.
*   **Save State Serialization:** The entire system bus and all sub-components support full state serialization via `serde`, allowing for human-readable snapshots of the system state in JSON format.

## 🧪 CI/CD & Automated Testing

Genteel is built on a foundation of aggressive, automated verification to ensure architectural integrity:

*   **Exhaustive M68k Testing:** Over 3,000+ randomized and exhaustive tests verify the M68k ALU and core instructions across all sizes (Byte, Word, Long) and edge cases.
*   **Z80 Torture Tests:** A specialized suite of "torture tests" verifies extreme architectural nuances of the Z80, including MEMPTR (WZ) leakage, R register wrapping, and EI interrupt shadowing.
*   **Security & Quality Audit:** A Python-based audit tool (`scripts/audit_tool.py`) is integrated into the development workflow to detect potential secrets, technical debt, and `unsafe` Rust patterns.
*   **Headless Validation:** Designed for CI environments, Genteel can run without a GUI, executing TAS-like input scripts and capturing state or screenshots for visual regression testing.
*   **Property-Based Testing:** Extensive use of the `proptest` crate to discover edge cases in CPU decoding and arithmetic logic.

## 🤖 AI-Driven Development

Genteel is designed from the ground up to be "agent-friendly":

*   **Agent Operational Context:** The project includes an `AGENTS.md` file that provides foundational mandates and architectural constraints specifically for AI language models contributing to the codebase.
*   **Instrumentable API:** Exposes a clean internal API for agents to drive the system, inject inputs, and analyze memory without the overhead of a traditional GUI.
*   **Deterministic Execution:** Emphasizes deterministic stepping of all components (M68k, Z80, APU, VDP) to ensure that AI-driven experiments are reproducible.
*   **Serialization-First Design:** Component states are accessible via a standardized `Debuggable` trait, making it easy for AI models to "see" the internal state of the machine.

## 🚀 Getting Started

### Prerequisites
*   **Rust:** [rustup.rs](https://rustup.rs/)
*   **Linux:** `sudo apt-get install build-essential libasound2-dev`
*   **Windows:** Build Tools for Visual Studio 2022 (with C++ workload)

### Build and Run
```bash
# Clone the repository
git clone https://github.com/segin/genteel.git
cd genteel

# Build release binary
cargo build --release

# Run a ROM
./target/release/genteel path/to/your/rom.zip
```

### Running the Test Suite
```bash
# Run all tests (M68k, Z80, VDP, etc.)
cargo test

# Run the security/quality audit
make audit
```

## 📜 License

This project is licensed under the MIT License.
