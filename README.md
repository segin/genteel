# Genteel

Genteel is an instrumentable, highly-verified Sega Mega Drive/Genesis emulator built in Rust. It is architected to be a high-performance bridge between **human creativity**, **automated verification**, and **artificial intelligence**.

Unlike traditional emulators, Genteel prioritizes **transparency** and **programmability**, making it a "one-stop shop" for developers debugging homebrew, CI/CD pipelines validating ROMs, and AI agents learning to interact with 16-bit hardware.

## 🛠 Pillar 1: Comprehensive Debugging

Genteel provides a data-rich, multi-window environment designed for deep system analysis:

*   **Integrated Debugging Suite**: A dedicated multi-window UI providing real-time visualization of:
    *   **VDP State**: Palette viewers (CRAM), Tile/Pattern viewers (VRAM), and full Scroll Plane renders (Plane A/B).
    *   **CPU Internal State**: Detailed register and flag displays for both the M68k and Z80.
    *   **Memory Hex Editors**: Live views of WRAM, Z80 RAM, VRAM, CRAM, and VSRAM.
    *   **Audio Visualizers**: Real-time FM (YM2612) and PSG (SN76489) parameter tracking and per-channel oscilloscope waveforms.
*   **Standard GDB Support**: A built-in GDB stub implementing the Remote Serial Protocol (RSP). Connect standard tools like `gdb` to set hardware breakpoints and step through M68k code.
*   **Execution Control**: Precision control with Pause, Resume, and Single-Step functionality synchronized across all debug windows.

## 🧪 Pillar 2: CI/CD & Automated Instrumentation

Built for the modern development lifecycle, Genteel ensures architectural integrity through aggressive automation:

*   **Headless Validation**: Run without a GUI in CI environments to execute TAS-like input scripts and capture system state or screenshots for visual regression testing.
*   **Massive Test Coverage**: 
    *   **3,000+ M68k Tests**: Exhaustive verification of the ALU and core instructions across all sizes and edge cases.
    *   **Z80 Torture Suite**: Verifies extreme architectural nuances like MEMPTR (WZ) leakage and EI interrupt shadowing.
*   **Deterministic by Design**: Component stepping is strictly deterministic, ensuring that automated experiments and bug reproductions are 100% reliable.
*   **Automated Builds**: Integrated GitHub Actions generate verified Linux and Windows release artifacts on every push.

## 🤖 Pillar 3: AI-Driven Development

Genteel is architected to be "agent-friendly," treating AI models as first-class users:

*   **Serialization-First Architecture**: The entire system state—from registers to the shared bus—is serializable via `serde`. This allows AI agents to "see" and "snap" the machine state in a structured JSON format.
*   **Agent Operational Context**: Includes `AGENTS.md`, providing foundational mandates and architectural constraints specifically for AI models contributing to or observing the system.
*   **Input Injection API**: A clean internal API allows agents to drive the system and inject inputs without the overhead of traditional HID emulation.
*   **Instrumentable Feedback**: Real-time visual and structural representation of internal states (PC, V-Counter, etc.) provides rich data for AI-driven observation and reinforcement learning.

## 🚀 Getting Started

### Prerequisites
*   **Rust (Edition 2021)**: [rustup.rs](https://rustup.rs/)
*   **Linux**: `sudo apt-get install build-essential libasound2-dev libudev-dev pkg-config`
*   **Windows**: Build Tools for Visual Studio 2022

### Build and Run
```bash
# Clone the repository
git clone https://github.com/segin/genteel.git
cd genteel

# Build release binary with GUI
cargo build --release --features gui

# Run a ROM
./target/release/genteel path/to/your/rom.md
```

### Running the Test Suite
```bash
# Run all verified tests
cargo test --features gui

# Run the security/quality audit
python3 scripts/audit_tool.py
```

## 📜 License

This project is licensed under the MIT License.
