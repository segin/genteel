# Implementation Plan: Rendering and Audio Expansion

## Phase 1: VDP Background Rendering [checkpoint: efca1fa]
- [x] Task: Analyze existing VDP rendering logic and identify why it outputs white. 4aebeee
- [x] Task: Implement Plane A background rendering. 4aebeee
    - [x] Write tests for Plane A tile fetching and rendering.
    - [x] Implement Plane A rendering in `src/vdp/mod.rs`.
- [x] Task: Implement Plane B background rendering. 4aebeee
    - [x] Write tests for Plane B tile fetching and rendering.
    - [x] Implement Plane B rendering in `src/vdp/mod.rs`.
- [x] Task: Conductor - User Manual Verification 'Phase 1: VDP Background Rendering' (Protocol in workflow.md) efca1fa

## Phase 2: Audio Channel Expansion
- [x] Task: Expand PSG implementation. 2eb2ebd
    - [x] Write tests for PSG square wave and noise channels.
    - [x] Implement missing PSG channels in `src/apu/psg.rs`.
- [x] Task: Expand YM2612 implementation. e2ba305
    - [x] Write tests for multiple YM2612 channels and operators.
    - [x] Implement missing YM2612 features in `src/apu/ym2612.rs`.
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Audio Channel Expansion' (Protocol in workflow.md)

## Phase 3: Final Integration and Testing
- [x] Task: Add non-interactive screenshot and duration support. 06b93af
    - [x] Add `image` crate dependency.
    - [x] Implement `save_screenshot` in `Emulator`.
    - [x] Add `--screenshot <path>` and improved `--headless` CLI support.
- [x] Task: Verify overall system stability and performance. 27e067d
- [ ] Task: Fix any remaining rendering or audio artifacts.
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Final Integration and Testing' (Protocol in workflow.md)

## YM2612 Parity Checklist [in progress]
- [x] Fix native YM sample cadence and keep EG updates on YM EG ticks only.
- [x] Fix direct DAC timing updates and frontend sample-rate/timing propagation.
- [x] Fix real slot/key-on ordering for `0x28`.
- [x] Fix algorithm routing, delayed `MEM`, and sample-before-phase/EG ordering.
- [x] Fix `MUL=0` handling in the phase generator.
- [x] Add LFO register handling, AMS, PMS, and replace PM shortcut with the reference-style PM table path.
- [x] Keep PM from contaminating EG/KSR and detune keycode selection.
- [x] Fix CSM Timer A auto-keying, retrigger behavior, and CSM key-off timing.
- [x] Add SSG-EG hold/loop/invert behavior and release-side inversion/termination fixes.
- [x] Fix SSG hold shape force-to-max XOR behavior.
- [x] Fix SSG loop restart behavior for max-attack transitions.
- [x] Fix detune wrap to stay in the YM 17-bit detune domain.
- [x] Fix CH3 special mode PM + detune to use the shared channel keycode on PM updates.
- [x] Replace remaining ad hoc operator frequency stepping with a closer `fc`/increment path to the reference core.
- [x] Reduce remaining operator output approximation gaps versus the reference tables and mixer behavior.
- [x] Fix YM2612 versus YM3438 read-port behavior, including discrete undefined-read decay.
- [x] Fix single-data-port bank selection so data writes follow the last addressed register bank.
- [x] Add profile-aware BUSY timing for address and data writes instead of one flat delay.
- [x] Fix YM3438 BUSY wait-cycle unit conversion so documented address/data windows are enforced in the same MCLK domain consumed by `step()`.
- [x] Move YM stepping to 42-MCLK internal cycles with per-channel subcycle clocking instead of only advancing all six channels at whole-sample boundaries.
- [x] Make DAC data/enable changes audible through the normal channel-6 sample pipeline instead of injecting write-time output deltas, with regression coverage for post-slot writes missing the current sample.
- [x] Defer FNUM high/block commits until the matching low-byte write for both normal channels and CH3 special frequencies.
- [x] Gate discrete ladder-effect mixer offsets by hardware profile so YM3438 mode does not inherit YM2612 DAC coloration.
- [x] Apply carrier quantization at operator output time so algorithm-7 op1 feedback history uses masked output like the reference core.
- [x] Remove non-hardware soft clipping from the final APU mix path and keep host delivery linear/saturated.
- [x] Replace callback lock-miss zero filling with last-sample hold so OS audio delivery does not click on brief producer/callback contention.
- [x] Normalize the FM/PSG blend so the final mix does not add 25% gain and clip same-sign chip output unnecessarily.
- [x] Preserve wider internal phase-modulation buses instead of truncating intermediate operator routing to 14-bit/i16 too early.
- [x] Gate carrier-output quantization masks by hardware profile so YM3438 mode does not inherit discrete-YM2612 DAC quantization.
- [x] Add direct regression coverage for the remaining hand-coded algorithm routing cases (`3` and `5`) to keep `MEM`/carrier wiring aligned with the reference.
- [x] Fix Timer B cadence to tick once every 16 YM samples instead of every sample.
- [x] Add the Genesis low-pass filter at the final APU output so YM/PSG audio does not go straight to the OS completely unfiltered.
- [x] Tighten the final low-pass stage to first-order Butterworth IIR coefficients instead of a rough RC shortcut.
- [x] Add direct regression coverage for the remaining delayed-edge algorithms (`0`, `1`, and `2`) to rule out hidden `MEM`/previous-sample routing bugs.
- [x] Make the hardware-facing YM port path reject data writes while BUSY is asserted instead of only reporting BUSY in status.
- [x] Make the public `Apu` FM write wrapper follow the same BUSY-gated hardware semantics instead of bypassing them through direct bank writes.
- [x] Audit register-write side effects for TL/AR/DR/SR/SL/RR/SSG updates against the reference and close any remaining gaps.
- [x] Apply active SSG-EG shape-write state changes immediately instead of waiting for the next sample update path.
- [x] Add direct regression coverage for live `B0` algorithm and feedback rewrites so running-channel `MEM` and op1 history stay aligned with the reference path.
- [x] Stop `Apu::set_timing` from resetting final low-pass history on every sync slice when timing is unchanged, while still repairing stale child timing state.
- [x] Make callback underruns reuse the last delivered stereo sample instead of snapping to zero on partial stereo starvation.
- [x] Pace GUI redraws by active video region so PAL playback is not hardwired to a 60 Hz host frame cadence.
- [x] Keep queue trimming stereo-aligned and stop dropping freshly generated temp-buffer samples in the main audio path.
- [x] Stage GUI audio uploads locally and flush them with non-blocking locks so the callback is not forced to wait behind whole-frame pushes.
- [x] Allow bounded multi-frame catch-up before render so audio production can recover from short GUI stalls.
- [x] Move emulation/audio production out of `RedrawRequested` and into the event-loop pacing path so rendering stalls do not directly starve the callback.
- [x] Latch YM sample state at sample start so mid-sample register writes do not skew later channel slots, current-sample stereo mixing, or the already-latched current EG tick.
- [ ] Run comparative runtime validation against known-problem titles and keep this checklist synced with observed failures.
