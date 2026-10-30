# Architecture Overview

This document serves as a critical, living template designed to equip agents with a rapid and comprehensive understanding of the codebase's architecture, enabling efficient navigation and effective contribution from day one. Update this document as the codebase evolves.

## 1. Project Structure

This section provides a high-level overview of the project's directory and file structure, categorised by architectural layer or major functional area. It is essential for quickly navigating the codebase, locating relevant files, and understanding the overall organization and separation of concerns.

```text
genteel/
├── src/
│   ├── lib.rs            # Library crate: the emulator core (every module below except gui)
│   ├── main.rs           # Binary crate: CLI, Emulator/scheduler, save states; uses the library
│   ├── gui.rs            # egui/winit/pixels front end (binary only, `gui` feature)
│   ├── tests_gui.rs      # GUI state tests (binary only)
│   ├── frontend.rs       # Keyboard-to-pad mapping and RGB565 framebuffer conversion
│   ├── audio.rs          # Host audio ring buffer and cpal consumer
│   ├── input.rs          # Controller input manager and input scripts (record/replay)
│   ├── wav_writer.rs     # WAV dump of the mixed output
│   ├── test_utils.rs     # Shared unit-test helpers (`cfg(test)`)
│   ├── cpu/              # M68k CPU: decoder, addressing, ops/, tests
│   ├── z80/              # Z80 sound CPU: op_general/op_cb/op_ed/op_index, tests
│   ├── apu/              # YM2612 FM, SN76489 PSG, blip buffer, state clamping
│   ├── vdp/              # VDP: ports, DMA, line timing, renderer
│   ├── memory/           # Bus, cartridge/SRAM, EverDrive mapper and I/O, Z80 bus view
│   ├── io/               # Controller ports and I/O registers
│   ├── debugger/         # Debuggable trait and the GDB RSP stub (gdb.rs)
│   └── bin/              # Stand-alone micro-benchmarks (*_bench.rs) using the library
├── benches/              # Criterion benchmarks (GUI ones need the `gui` feature)
├── tests/                # Integration tests (Rust) and audit-tool tests (Python)
├── fuzz/                 # cargo-fuzz crate: one target per fuzz_targets/*.rs
├── proptest-regressions/ # Saved proptest failure seeds
├── docs/                 # Hardware notes, scripting guide, audit task list
├── scripts/              # Audit tool and release build helper
├── conductor/            # Planning workspace for the Gemini Conductor workflow
├── .github/workflows/    # CI (ci.yml) and tag-driven releases (release.yml)
├── Cargo.toml            # Rust project manifest
├── README.md             # Project overview for humans
├── AGENTS.md             # AI agent operational context (CLAUDE.md, GEMINI.md link here)
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
                                                  +--> [Z80 (sound CPU)] --> [APU (YM2612, PSG)]
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
Description: Coordinates the main emulation loop, steps the M68k CPU, steps the Z80 co-processor, advances the bus/VDP/APU timing, checks for H/V interrupts, and manages bus contention. The scheduler (`main.rs`: `run_system` / `run_scanline` / `run_cpu_batch_static`) sizes each 68k batch to the master clocks left in the VDP's current line (3420 MCLK per line, 7 MCLK per 68k cycle, as in Genesis Plus GX) and takes line and frame boundaries from the VDP's own counters, which carry the last instruction's overshoot into the next line. Bus/IRQ/Z80/APU catch-up is one helper (`sync_system`, with `update_m68k_irqs` raising/cancelling IRQ 6 and 4); the Z80 VINT is raised at the start of the first blanking line. An optional `ExecLimit` (breakpoints, instruction budget) is checked before every 68k instruction, so a debugger can stop mid-frame and resume exactly where it left off. Completed active scanlines are rendered from the VDP timing path at line end. The GUI paces frames at the emulated rate (59.92 Hz NTSC, 49.70 Hz PAL); headless runs (`--headless N`) have no frame cap and run as fast as the host allows.
Technologies: Rust

### 3.2. M68k CPU
Name: CPU (`src/cpu/`)
Description: Implements the main processor, a Motorola 68000 (M68k). Responsible for fetching, decoding, and executing instructions from the game ROM and RAM. Each instruction is executed as the sequence of bus cycles the hardware performs (program fetches, data reads/writes, internal idle clocks), in hardware order; the clock count falls out of that sequence. Bus helpers return `Result<_, AddressFault>`, so a word/long access to an odd address (or a branch to an odd target) aborts the instruction at the same point the chip does and takes the 7-word group 0 address-error frame; a fault while stacking that frame halts the CPU until reset (`halted`, which ignores interrupts; STOP uses the separate `stopped` state). Opcodes are decoded once into a process-wide 64K-entry table (no per-PC decode cache, so bank switches, self-modifying code and save states need no invalidation); invalid encodings, including disallowed effective-address modes, decode as illegal instructions. Trace, SR/CCR bit masking and per-exception timing are modelled. TAS write-back is a bus property (`MemoryInterface::tas_write_back`), which the Mega Drive `Bus` turns off. The core is checked against the SingleStepTests 680x0 data by an ignored harness (`src/cpu/tests_harte.rs`), which compares registers, SR, stacks, memory and cycle counts.
Technologies: Rust

### 3.3. Audio Processing Unit (APU)
Name: APU (`src/apu/`)
Description: Contains the Yamaha YM2612 FM synthesizer and the Texas Instruments SN76489 PSG; the Zilog Z80 sound co-processor that drives them lives in `src/z80/` and is described at the end of this section. The APU timing is configurable by video region and output sample rate, with both PSG and YM2612 clocking derived from the active Genesis master clock through the shared `Apu::set_timing` path. That timing reconfiguration now preserves final low-pass filter history when the requested timing is unchanged, instead of resetting the mixed output state on every normal sync slice, while still repairing stale child clock/sample-rate state if needed. The YM2612 core is being moved toward Genesis Plus GX behavior with native-sample timing, 42-MCLK internal-cycle stepping with per-channel subcycle clocking, sample-latched YM state so mid-sample register writes do not skew later channel slots, current-sample stereo mixing, or the already-latched current EG tick, DAC state changes that now become audible through the channel-6 sample pipeline instead of write-time blip injection, EG-tick scheduling, reference-style LFO PM tables, more accurate SSG-EG state handling, immediate active SSG-EG shape-write application, CSM keying behavior, tighter detune/phase stepping, selectable discrete-YM2612 versus YM3438 read-port behavior, single-data-port bank selection that follows the last addressed register group, profile-aware BUSY timing for address and data writes, including correct YM3438 wait-cycle conversion into the Genesis MCLK domain consumed by `step()`, data-port writes that require the port's A1 line to match the last-addressed bank and are otherwise ignored (writes are applied regardless of BUSY, matching real hardware and BlastEm; BUSY affects only the status read), deferred FNUM/block commit semantics that apply on the matching low-byte write, Timer B’s `/16` sample cadence, profile-gated DAC ladder coloration so YM3438 mode does not inherit discrete-YM2612 output offsets, profile-gated carrier quantization masks, wider internal phase-modulation buses that avoid premature 14-bit truncation in operator routing, and operator-time carrier masking so algorithm-7 feedback history sees quantized output like the reference core. Final APU mixing to the host is now linear/saturated rather than running through an artificial soft-clipping stage, the FM/PSG blend is normalized instead of boosted, the mixed output now runs through a Genesis-style first-order Butterworth low-pass filter before host delivery, the OS callback holds the last delivered sample on buffer lock misses and stereo underruns (decaying it toward zero so a stopped producer cannot park a DC level) instead of snapping to zero, queue trimming stays stereo-aligned instead of severing left/right pairs, freshly generated temp-buffer samples are not dropped just because the per-frame queue hit a cap, GUI-side audio uploads are staged locally and flushed with non-blocking locks, and emulation/audio production now runs from the event-loop pacing path with a small bounded catch-up burst before redraw so rendering stalls do not directly starve the callback while the GUI still follows the active NTSC/PAL frame cadence. The Z80 implementation handles architectural nuances like MEMPTR (WZ Register), R Register refresh (once per M1 fetch, bit 7 stable), the Q register used by SCF/CCF, ignored DD/FD prefixes (a run of them is folded into one step, at most 16 per step, with interrupts held off until the run ends), and EI interrupt shadowing, and is checked against the SingleStepTests Z80 data by an ignored harness (`src/z80/tests_sst.rs`). The Z80 INT line is modelled as a level held for one scanline (`Z80::assert_int_for`) and sampled at instruction boundaries inside `Z80::step`, which returns the acknowledge T-states so they are charged to the Z80 cycle budget; while the 68000 holds BUSREQ or reset the Z80 does not step, so no interrupt is delivered and the line only times out.

Audio signal path (current state, superseding the callback notes above): the PSG (`apu/psg.rs`) follows Genesis Plus GX `psg.c` and MAME `segapsg_device` for the integrated Sega chip: counters tick once per 16 PSG clocks (240 MCLK), so a tone of period N sounds at `PSG_clock / (32·N)`; period 0 behaves as 1; the noise register is the 16-bit Sega LFSR (seed 0x8000, taps 0x0009, feedback into bit 15) shifted only on the rising edge of the noise divider flip-flop, with rate 3 driven by tone 2's reloads; data bytes to the latched noise register rewrite its control and reset the LFSR; channel output is unipolar and its DC is removed by a one-pole DC blocker in `Apu::generate_sample` (the role of blip_buf's built-in high-pass / the output coupling capacitor). `BlipBuf` (`apu/blip_buf.rs`) maps source clocks to output samples with an absolute 64.64 fixed-point timebase (no lost time while a source is silent, no per-delta division), uses a 256-phase, 16-tap windowed-sinc kernel with blargg's phase orientation and a cutoff at 0.875 × output Nyquist (blargg uses 0.9), and on a timing change keeps its settled level so sources' delta baselines stay valid. The YM2612 shares one F-number high latch for $A4-$A6 and one for $AC-$AE (GPGX `fn_h`, Nuked `reg_a4`/`reg_ac`), and discrete-chip reads of ports 1-3 return the last port-0 status for Nuked's 300000 internal cycles, then 0. `Apu::write_fm_data` has the hardware single-data-port semantics. Restored APU state (save states, debugger `write_state`) is range-clamped field by field (`apu/state_clamp.rs`) and `BlipBuf` validates/repairs its buffer on deserialization. Host output (`audio.rs`) is a lock-free SPSC ring of packed stereo frames; the cpal callback (`AudioConsumer`) steers the fill level to 40 ms with ±0.5 % dynamic rate control, primes before playback, decays a held sample on underrun and skips ahead only beyond a 100 ms bound. `WavWriter` refuses existing files and symlinked escapes, stops cleanly at the 4 GiB RIFF limit or on the first I/O error, and always finalizes a header that matches the bytes written.
Technologies: Rust

### 3.4. Video Display Processor (VDP)
Name: VDP (`src/vdp/`)
Description: Responsible for rendering the graphics. It manages video RAM (VRAM), sprites, backgrounds, and generates the video output. Scanline completion is timing-driven inside `Vdp::tick`, and the renderer uses an internal Sprite Attribute Table mirror to evaluate sprites instead of treating VRAM as the sole source of visible sprite state.

- **Line timeline** (`src/vdp/timing.rs`): every line is 3420 MCLK. Line-relative MCLK 0 is the point where the V counter increments and HINT fires (H counter 0xA5 in H40, 0x85 in H32, as in Genesis Plus GX `hvc.h` and BlastEm). The H counter, HBLANK flag (MCLK 228-871 / 280-859), VINT (MCLK 788 / 770 of the first blanking line), the FIFO/DMA access slots and the renderer all use this origin. Pixel 0 of a line is emitted at MCLK 988 (H40) / 1000 (H32); its last 16/14 pixels are emitted during MCLK 0-127/0-139 of the next line. A line is rendered when its active display starts; later CRAM/VSRAM/register writes re-render or recomposite only the pixels the beam has not emitted yet. VBLANK is set from the first blanking line to the last line of the frame. `tick` processes spans of any length line by line.
- **Mid-line updates**: the plane/sprite layer buffers of the current line are cached, so colour-only changes (CRAM, backdrop, column mask) are recomposited without re-rendering the layers.
- **Status and interrupts**: a 68k status read clears only the sprite overflow/collision flags; VINT and HINT stay pending until acknowledged, and a pending HINT is part of the save state. `peek_status` is the side-effect-free view for debuggers and the GUI. The Z80 INT line is asserted in the CPU slice in which the VDP passes the VINT point of the first blanking line (`Vdp::take_z80_vint`), whether or not VINT is enabled, as Genesis Plus GX does. Setting register 0 bit 1 (M3) latches the HV counter until the bit is cleared (Genesis Plus GX `vdp_reg_w`).
- **Access slots, FIFO and DMA**: the FIFO and DMA use the access slots of the timeline: the 16 (H32) / 18 (H40) external slots during active display, and during blanking (or with the display off) every slot except the refresh slots, spread evenly so 166 / 204 remain. The FIFO is served first. A VRAM word (FIFO entry or 68k DMA) and a copied byte take two slots, a fill byte or a CRAM/VSRAM word one; a 68k DMA to CRAM/VSRAM in blanking also loses the slot after each refresh slot. This reproduces the Genesis Plus GX `dma_timing` rates per line. A data-port write to a full FIFO stalls the 68k until the oldest entry has been written (`Vdp::fifo_wait_mclk`). The DMA kind is latched from register 23 at the CD5 control write. A 68k DMA freezes the 68k (`Bus::dma_active`), and the second word of a control-port long write that started one is held until the DMA ends. A fill (armed at the control write, started by the next data-port write after the two slots that write takes) or a copy leaves the 68k running and only sets the status DMA busy flag until it ends.
- **Sprites**: one sprite more than the per-line count limit (16 / 20) sets the sprite overflow flag; reaching the dot limit (256 / 320) arms the X=0 sprite mask for the next line. Re-rendering a line mid-line does not set the overflow and collision flags again.
- **SAT cache**: VRAM writes (CPU, DMA, fill, copy) into the sprite table block selected by register 5 update the cache; changing register 5 does not reload it. Y/size/link come from the cache, pattern/attribute and X from VRAM.
- **Interlace**: register 12 LSM bits select interlace mode 1 or 2; the odd-field status flag toggles every frame while interlaced, the HV counter applies the interlace transformation, and mode 2 renders the current field with 8x16 tiles and 10-bit sprite Y.
- **Not modelled**: Mode 4 (SMS compatibility) rendering and the rule that registers 11-23 ignore writes while M5 is clear; the 128 KiB VRAM mode (register 1 bit 7); the reduced sprite limits after the display is re-enabled late in HBLANK (Genesis Plus GX drops to 128 sprite pixels); a Z80 status read clearing the VINT flag; the Z80 being held off the bus while it accesses the VDP during a 68k DMA (its control word is held like the 68k's); 68k DMA sources in `$A00000-$DFFFFF` (Z80 area, I/O, VDP), which read `0xFFFF`; the exact positions of the refresh slots and the plane A/window boundary glitch as Genesis Plus GX draws it (a duplicated plane A column; the H40 glitch here draws an extra window tile).
Technologies: Rust

### 3.5. Memory & Bus
Name: Memory (`src/memory/`)
Description: Implements the memory bus and mapping via the `MemoryInterface` trait. The `SharedBus` wrapper allows components to share the `Bus` state. Supports full state serialization/deserialization via `serde`; restored work RAM and Z80 RAM must have their exact sizes or the state is rejected.
- **Cartridge** (`src/memory/cart.rs`): the `Sram` window comes from the ROM header (`RA` at 0x1B0, with the Genesis Plus GX address fix-ups); headerless ROMs of up to 2 MiB get 64 KiB at 0x200000. SRAM is mapped at power-on only if it lies above the ROM image; `$A130F1` bit 0 maps it and bit 1 write-protects it. Reset keeps its contents and restores the power-on mapping. ROMs over 4 MiB get the Sega bank mapper (`$A130F3-$A130FF`, slot 0 fixed) plus a linear view up to 0x9FFFFF. VDP DMA reads go through the same cartridge mapping. `"SEGA SSF"` ROMs use the EverDrive mapper (`everdrive.rs`), which owns the only copy of the image as its backing memory (`Bus::rom` stays empty; `Bus::rom_bytes`/`has_rom` read either). Writes through its `W` bit are tracked in 4 KiB pages, and the original bytes of overwritten ROM pages are kept so `Bus::take_rom_image` returns the image as loaded.
- **EverDrive I/O** (`src/memory/everdrive_io.rs`): with the SSF mapper enabled, `$A130D0-$A130DF` is the Mega EverDrive core device, following the QEMU `md_everdrive` model LinuxMD was developed against. `$A130D0` is the data FIFO (a word access pops/pushes one byte in the low lane; a byte read of `$A130D1` pops, a byte write to either address pushes), `$A130D2` is the status (bits 0-10 RX level, bit 15 RX non-empty; TX is always ready) and `$A130D6` a writable 16-bit millisecond counter. Commands are framed `'+'`, `~'+'`, cmd, ~cmd plus big-endian arguments: STATUS, USB_WRITE, DISK_INIT, F_DIR_LD, F_DIR_SIZE, F_DIR_GET, F_FOPN, F_FRD, F_FCLOSE, F_FPTR, F_AVB; responses are immediate. The SD card is a host directory set with `EverdriveIo::set_sd_root` (`--everdrive-sd`), read-only (write modes fail) and confined: a leading `/` is stripped, `..`/absolute/drive components, NUL and backslashes are refused, and the canonical path must stay under the canonical root, so symlinks cannot escape; listings are sorted by name and skip escaping entries. Guest-controlled sizes are bounded: the RX FIFO holds 2047 bytes (the largest level the 11-bit field reports), strings keep 1 KiB, staged responses 64 KiB, USB output waiting for the host 64 KiB, and an `F_FRD` of any length is a queued file/zero segment streamed into the FIFO as the guest drains it. The timer is advanced from `Bus::tick` in master clocks (NTSC or PAL rate), never host time. USB output is collected by `take_usb_output` and host input enters with `push_host_input`, only between commands so it cannot split a response; the binary's `Emulator::pump_usb_serial` moves both once per frame (stdout, and in headless mode stdin through a reader thread and a bounded channel). The serialized state covers the parser, FIFOs, queued segments, status, disk/directory/open-file path and position and the timer; host handles are not saved, and `EverdriveIo::adopt_host` hands the SD directory to a restored device, which reopens its file by path. Malformed states are clamped to the same bounds.
- **Decode**: VDP ports respond wherever `(addr & 0xE700E0) == 0xC00000`, the same for every access size; the PSG sits on the odd bytes of 0xC00010-0xC00017. The 68k sees the 8-bit Z80 window with word reads duplicating the byte and word writes storing only the high byte; Z80 RAM is mirrored at 0xA02000. BUSACK reads as granted, and the 68k reaches Z80 RAM, only while the bus is requested and the Z80 is out of reset (Genesis Plus GX `zstate == 3`). Asserting or releasing Z80 reset also resets the YM2612. The Z80 reaches the VDP/PSG through 0x7F00-0x7FFF; banking into its own window locks it up until its next reset (`Bus::z80_can_run`). The scheduler borrows the shared bus once per Z80 slice and runs the Z80 against a `Z80BusView` of it through `Z80::with_bus`; `Z80Bus` (one borrow per byte) remains for the debugger and tests.
- **Region**: the header region field (or `Bus::set_region_override`) sets the version register and VDP PAL timing on ROM load. `Bus::restore_rom` re-attaches ROM bytes after a save-state restore without re-deriving SRAM, banks or region.
Technologies: Rust (Trait Objects, Interior Mutability, Serde)

### 3.6. I/O and Debugger
Name: I/O (`src/io/`) & Debugger (`src/debugger/`)
Description: Handles all input and output (game controllers), and provides a GDB Remote Serial Protocol (RSP) interface to allow external debuggers to connect to the emulator. I/O registers repeat on both byte lanes; data reads return `(latch & ctrl) | (pins & !ctrl)`, and TH follows the latch only while configured as an output. The 3- and 6-button pad protocol follows Genesis Plus GX `gamepad.c`: the 6-button counter advances on TH rising edges the I/O chip drives and times out after about 1.5 ms of 68k cycles, and a low TH released to an input reads high at once but the pad keeps its TH-low data for 172 master clocks (25 68k cycles); both timers are ticked from `Bus::tick`. No light gun is emulated, so the VDP HV counter is never latched by TH, only by register 0 M3 (`Vdp::latch_hv_counter` is the hook for such a device). Input scripts escape backslashes, newlines and carriage returns in the command field; recording stores the effective input of every frame under its own frame index. The GDB stub (`debugger/gdb.rs`) binds to loopback only, parses packets as bytes with a persistent incremental parser (pipelined and split packets, 0x03 breaks), rejects non-ASCII payloads and non-hex checksums, writes replies completely, resets authentication/no-ack/breakpoints per connection, caps `m` reads to the advertised PacketSize (hex) and closes only after the D reply. `GdbServer::service` is the single command loop for headless and GUI modes; `Emulator::poll_gdb`/`run_gdb_slice` run the full system, halt on attach, stop on breakpoints and 0x03, single-step one instruction and send stop replies. Debugger memory access uses the side-effect-free `Bus::peek_byte`/`poke_byte`.
Technologies: Rust, GDB RSP

## 4. Data Stores

### 4.1. Game ROMs
Name: Primary Game ROM
Type: File (binary/zip)
Purpose: Read-Only Memory where the game's code and data are stored. Loaded into the emulator's memory at the start. The loader opens the canonicalized path once, extracts the first ROM entry from zips (size-limited), and de-interleaves SMD dumps. ROM roots (`add_allowed_path`) are opt-in: with none configured the file the user named is loaded.

### 4.2. Internal Memory
Name: System RAM
Type: In-memory buffers
Purpose: The emulator manages several internal memory regions: Work RAM (64KB for M68k), Video RAM (64KB for VDP), and Sound RAM (8KB for Z80).

### 4.3. Battery Saves
Name: Cartridge SRAM (`.srm` next to the ROM)
Type: File (binary)
Purpose: Same layout as Genesis Plus GX: up to 64 KiB, the byte at CPU address `A` stored at offset `A & 0xFFFF`, untouched bytes `0xFF`, truncated after the last non-`0xFF` byte; nothing is written while SRAM is untouched. Files from older builds (even-address bytes packed and zero-filled, exactly the window size) are recognised when unambiguous: an all-zero file is ignored and a packed even-lane file is expanded; other legacy files are loaded as-is.

### 4.4. Save States
Name: Save Games
Type: File (binary/JSON)
Purpose: Compact JSON (`serde`) of the machine state in slots `.s0`-`.s9` (and `.auto`) next to the ROM, written atomically. The ROM image is not stored: on load the bytes already in memory are re-attached with `Bus::take_rom_image`/`Bus::restore_rom` (SRAM, mapper banks, the SSF mapper's written RAM pages and region come from the state; states without those pages still load), the Z80 is reconnected to the restored bus, and session state (GDB, WAV dump, input recording/script, debug/pause flags, input mapping, ROM path, the EverDrive SD directory and USB serial link) is kept. The EverDrive device state is saved with serde defaults; its open file is reopened by path. Derived `.srm`/state paths append the extension when swapping it would name the ROM. SRAM is written atomically on every exit path (the emulator saves it on drop).

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
CI/CD Pipeline: GitHub Actions. `ci.yml` runs once per change (pushes to `main` and pull requests into `main`) with `RUSTFLAGS=-Dwarnings`: `cargo fmt --check` (crate and fuzz crate), `cargo clippy --all-targets`, `cargo test` plus the Python tests, a headless `--no-default-features` build and `--all-targets` check without GUI system libraries, and a `--locked` check of every fuzz target. `release.yml` runs on `v*` tags (or manually), builds Linux and Windows release archives with build caching and publishes them as a GitHub Release; only the publish job gets `contents: write`. Warnings are denied by CI flags rather than `#![deny(warnings)]` in the crate roots, so newer local toolchains do not break local builds. Both crate roots `#![forbid(unsafe_code)]`.
Monitoring & Logging: `log` and `env_logger` crates for structured output (`RUST_LOG=debug`)

## 7. Security Considerations

Authentication: The GDB stub binds to 127.0.0.1 only and every command except detach/kill/interrupt requires `monitor auth <secret>` first. The secret is `--gdb-password`/`--gdb-password-file`, or a random 128-bit token printed at startup. Comparison is constant-time (`subtle`); after 10 failed attempts authentication is locked out for 10 seconds. Authentication, no-ack mode and breakpoints are reset for every connection.
Authorization: N/A (an authenticated client has full debugger access)
Data Encryption: N/A
Key Security Tools/Practices:
- Bounds-check all memory accesses (emulator runs untrusted ROM code).
- Handle invalid/malformed instructions gracefully.
- The debugger interface uses local network sockets - restrict access appropriately.
- Run the audit tool (`python3 scripts/audit_tool.py`) periodically to detect secrets and unsafe patterns.

## 8. Development & Testing Environment

Local Setup Instructions: `cargo build` (debug) or `cargo build --release`
Toolchain and GUI stack: `rust-version` is 1.92, the minimum of the current dependency set (`egui 0.35`). The front end uses `winit 0.30` (an `ApplicationHandler`; the window, `pixels` surface and egui state are created on `resumed`), `pixels 0.17` for the scaled framebuffer, `egui`/`egui-winit`/`egui-wgpu 0.35` for the overlay (all three plus `pixels` share `wgpu 29`), `cpal 0.18` for audio, `gilrs 0.11` for gamepads and `rfd 0.17` (XDG portal) for file dialogs. egui is held at 0.35 because 0.36+ needs `wgpu 30`, which `pixels` does not support yet. The menu bar is a top `Panel` in a full-viewport root `Ui`, and debug windows are constrained to the area below it.
Testing Frameworks: `cargo test` (unit/integration), `proptest` (property-based tests; every suite sets its case count), `cargo-fuzz` (fuzzing, nightly; `cargo check --manifest-path fuzz/Cargo.toml --bins` works on stable). The core's unit tests live in the library crate and run once; the binary's tests cover the scheduler, save states, CLI and GUI state. Timing-only benchmarks are `#[ignore]`d (`cargo test -- --ignored` runs them), as are the SingleStepTests harnesses that need external data.
Code Quality Tools: `cargo clippy`, `cargo fmt`, `make audit` (custom audit script)

## 9. Future Considerations / Roadmap

- **M68k Implementation**: Completed full instruction set and addressing modes; bus-cycle-accurate timing and address-error handling validated against SingleStepTests (1,000,057 of 1,000,060 cases, all cycle counts).
- **APU Implementation**: Initial Yamaha YM2612 FM and SN76489 PSG support implemented; fixed test regressions.
- **AI Agent API**: Expanded scripting engine with memory/register manipulation commands (READ/WRITE/ASSERT).
- **Controller Support**: 3-button and 6-button controller support implemented.
- **GDB Support**: RSP stub with breakpoints, single-step, interrupt and stop replies, running the full system in headless and GUI modes.
- **Accuracy Improvements**: Moved VBlank/HBlank/LineCounter management into VDP `tick` for better cycle accuracy.
- **Accuracy Improvements**: Rendering ownership is now centered in `Vdp::tick` for completed scanlines, with SAT mirroring for sprite evaluation and region-aware APU timing configuration.
- **32X Expansion**: Future goal (dual SH2 cores, Master/Slave sync, 32X VDP).

## 10. Project Identification

Project Name: genteel
Repository URL: https://github.com/segin/genteel
Primary Contact/Team: N/A
Date of Last Update: 2026-10-29

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
