# Genteel

Genteel is an instrumentable Sega Mega Drive/Genesis emulator designed to be driven by AI language models. The purpose of this program is to enable automated testing of Genesis software in a "native" M68k environment.

## Features

*   **Instrumentable:** Genteel will expose an API to allow AI language models to drive the emulator, sending input and inspecting memory.
*   **Accurate Emulation:** Strives for accurate emulation of the Sega Mega Drive/Genesis hardware to provide a reliable test environment.
*   **Cross-Platform:** Built with Rust, Genteel is designed to be cross-platform.

## Getting Started

(Coming soon)

## Building from Source

To build Genteel, you will need the Rust toolchain installed.

### Linux
Install dependencies (Ubuntu/Debian example):
```bash
sudo apt-get install build-essential libasound2-dev
```
Then build:
```bash
cargo build --release
```

### Windows
1.  Install the **Rust toolchain** from [rustup.rs](https://rustup.rs/).
2.  Install **Build Tools for Visual Studio 2022** (available via the [Visual Studio Installer](https://visualstudio.microsoft.com/downloads/)). In the installer, select the "Desktop development with C++" workload.
3.  Open a terminal (PowerShell or Command Prompt) and run:
    ```powershell
    cargo build --release
    ```

### macOS
```bash
cargo build --release
```

## Running Tests

Genteel features a comprehensive test suite including unit tests, property-based tests, and regression tests.

```bash
cargo test
```

## Code Quality & Security

The project includes a standalone audit tool (`audit_tool.py`) to check for security issues and code quality. It scans the codebase for:

*   **Potential Secrets:** API keys, private keys, passwords.
*   **Technical Debt:** Unresolved `TODO`, `FIXME`, or `XXX` tags.
*   **Unsafe Code:** Usage of `unsafe { ... }` blocks in Rust.

To run the audit:

```bash
python3 audit_tool.py
```

This will generate reports in the `audit_reports/` directory:
*   `audit_reports/findings.json`: Detailed findings.
*   `audit_reports/RISK_REGISTER.csv`: A summary of findings.

## Status

Phase 4: System Integration is currently in progress.
- [x] M68k CPU Core (Instruction set complete)
- [x] Z80 CPU Core (Architectural nuances & Torture tests complete)
- [x] Unified Memory Bus (ROM, RAM, VDP, I/O)
- [x] Core Integration (M68k & Z80 sharing the same bus)

## License

MIT
