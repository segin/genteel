# AGENTS.md

This document provides AI-specific operational context for working on the `genteel` project. For general project information, see [README.md](README.md). For architecture details, see [ARCHITECTURE.md](ARCHITECTURE.md).

## Quick Reference

- **Language**: Rust
- **Build**: `cargo build`
- **Test**: `cargo test`
- **Run**: `cargo run`
- **Check**: `cargo clippy`
- **Format**: `cargo fmt`

## Agent Operating Principles

### 1. Keep ARCHITECTURE.md in Sync

The `ARCHITECTURE.md` file is the **source of truth** for the project's design. When making architectural changes:

1. **Read ARCHITECTURE.md first** before making significant changes
2. **Update ARCHITECTURE.md** whenever you modify the project's structure or design
3. **Update the "Date of Last Update" field** when modifying ARCHITECTURE.md

### 2. Commit and Push on Task Completion

At the end of each successfully completed task:

1. Commit all changes with a clear, descriptive commit message
2. Immediately push to the remote repository (`git push`)

### 3. Aggressive Testing

All new features, especially CPU opcodes, must be accompanied by comprehensive tests:

- **Unit tests**: Standard operation and edge cases
- **Property-based tests**: Use `proptest` crate for wide input coverage
- **Flag coverage**: 
  - M68k: C, V, Z, N, X flags
  - Z80: S, Z, Y, H, X, P/V, N, C flags (must include undocumented X/Y)

### 4. Z80 Architectural Integrity

Agents must implement and verify the following Z80 nuances for high compatibility:

- **MEMPTR (WZ)**: Must be updated during 16-bit loads, block ops, and bit tests.
- **Undocumented Flags (X/Y)**: Must leak from MEMPTR or Effective Address during `BIT` operations.
- **R Register**: Bit 7 must be stable during fetch; only lower 7 bits increment.
- **Interrupts**: `EI` instructions must have a 1-instruction shadow. `IM 2` must use a bus-provided vector.

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

## Code Style

- Use `cargo fmt` before committing
- Use `cargo clippy` to check for common issues
- Prefer functional patterns where possible
- Document public APIs with rustdoc comments
- Keep functions small and focused

## Security Considerations

- Bounds-check all memory accesses (emulator runs untrusted ROM code)
- Handle invalid/malformed instructions gracefully
- The debugger interface uses local network sockets - restrict access appropriately

## Agent Capabilities

AI agents working on this project can:

- **Develop the emulator**: Implement features from the roadmap in ARCHITECTURE.md
- **Play games**: Send controller inputs to test game behavior
- **Perform targeted testing**: Test specific functionality
- **Fuzzing**: Generate random inputs to test robustness
- **Analyze game states**: Read emulator memory to analyze internal state

---

*This file follows the [AGENTS.md](https://agents.md/) specification.*
