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

# Build release binary (the GUI is a default feature)
cargo build --release

# Run a ROM
./target/release/genteel path/to/your/rom.md
```

### Mega EverDrive programs and LinuxMD
ROMs whose header says `"SEGA SSF"` get the Mega EverDrive mapper (the whole
4 MB cartridge window as RAM) and its I/O device at `$A130D0`: the command
mailbox, the USB serial port and the 1 kHz millisecond timer.

* `--everdrive-sd <dir>` serves `<dir>` as the SD card, read-only. Guest paths
  are relative to it; `..`, and symlinks leading out of it, are refused.
  Without it the disk commands report an error and USB serial still works.
* USB serial output goes to stdout. In headless mode stdin is the serial input
  (read on a separate thread, so the emulator never waits for it); the GUI sends
  no serial input.
* The timer counts emulated time, so runs are deterministic.

To boot [LinuxMD](https://github.com/LinuxMD/linuxmd), put the files its
`runqemu.sh` uses in one directory and load `u-boot.bin` from it as the ROM:
```text
linuxmd-sd/
├── u-boot.bin     # loaded as the ROM; its bootcmd loads the kernel from the SD card
├── vmlinux.lz4    # kernel (everdrive disk load 0x1000 vmlinux.lz4)
└── m68k.erofs     # root file system (/dev/edblk)
```
```bash
cargo build --release
./target/release/genteel --headless 3600000 --everdrive-sd linuxmd-sd linuxmd-sd/u-boot.bin
```
Headless mode has no frame cap: it runs as fast as the host allows, which is
the quickest way to boot. `--headless N` stops after N frames (3,600,000 is
about 16.7 emulated hours at 60 Hz); stop earlier with Ctrl-C. The console is
the USB serial port on stdout/stdin (`console=ttyED0`), and the kernel's VDP
console is shown in GUI mode, which runs at real-time speed. Use a release
build: debug builds are many times slower.

### Running the Test Suite
```bash
# Run all verified tests
cargo test

# Run the security/quality audit
python3 scripts/audit_tool.py
```

#### 68000 SingleStepTests (optional)
The M68k core can be checked against the
[SingleStepTests 680x0](https://github.com/SingleStepTests/680x0) data set. The
harness is ignored by default because the data is external:
```bash
git clone --depth 1 https://github.com/SingleStepTests/680x0.git
mkdir harte && for f in 680x0/68000/v1/*.json.gz; do gunzip -c "$f" > harte/$(basename "${f%.gz}"); done
HARTE_DIR=$PWD/harte cargo test --release --lib cpu::tests_harte::harte_run -- --ignored --nocapture
# HARTE_ONLY=ASR limits the run to matching files; HARTE_SAMPLES=N sets failures printed per file;
# HARTE_EXC=1 also prints exception-case failures; HARTE_CYCLES=1 counts cycle-count mismatches
# as failures (otherwise they are reported as a separate tally).
```

## 📜 License

This project is licensed under the MIT License.
