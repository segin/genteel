# Comprehensive Codebase Audit Findings

**Date:** 2026-02-02T20:07:55.730451
**Commit:** a1b42dfa8bf71c3b143e133de42d855b37f15786

## Metrics
- **Files:** 399
- **Dependencies:** 322
- **Secrets Found:** 1
- **Tests:** Fail

## Detailed Findings
### [F-001] False Positive: Audit Tool Secret Pattern
- **Severity:** Info
- **Location:** `audit_tool.py:167`
- **Description:** The audit tool detected its own regex pattern.
- **Remediation:** Rotate secret and remove from history.

### [F-002] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/libcc-a62066ce084a8d02.rlib:1459`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-003] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/libcc-a62066ce084a8d02.rmeta:1439`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-004] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/libjobserver-25adbc36fdf7bdfe.rlib:161`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-005] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/libjobserver-25adbc36fdf7bdfe.rlib:166`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-006] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/libjobserver-25adbc36fdf7bdfe.rmeta:153`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-007] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/libjobserver-25adbc36fdf7bdfe.rmeta:158`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-008] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/liblibfuzzer_sys-691fbdf079f0efb7.rmeta:94`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-009] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/liblibfuzzer_sys-691fbdf079f0efb7.rmeta:95`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-010] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/libsyn-f935f9f7c4762fcb.rlib:8107`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-011] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `fuzz/target/debug/deps/libsyn-f935f9f7c4762fcb.rmeta:8087`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-012] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `src/main.rs:179`
- **Description:** // Execute any pending DMA (TODO: proper cycle-accurate DMA)
- **Remediation:** Address the comment.

### [F-013] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `src/z80/mod.rs:873`
- **Description:** // TODO: I/O implementation
- **Remediation:** Address the comment.

### [F-014] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `src/z80/mod.rs:878`
- **Description:** // TODO: I/O implementation
- **Remediation:** Address the comment.

### [F-015] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `src/z80/mod.rs:1082`
- **Description:** // TODO: I/O implementation
- **Remediation:** Address the comment.

### [F-016] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `src/z80/mod.rs:1094`
- **Description:** // TODO: I/O implementation
- **Remediation:** Address the comment.

### [F-017] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `src/z80/mod.rs:1357`
- **Description:** let io_val = 0xFF; // TODO: Real IO
- **Remediation:** Address the comment.

### [F-018] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `src/z80/mod.rs:1393`
- **Description:** // TODO: Real IO
- **Remediation:** Address the comment.

### [F-019] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:48`
- **Description:** // Miscellaneous. (TODO: Determine if these should be signed or unsigned.)
- **Remediation:** Address the comment.

### [F-020] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:224`
- **Description:** // TODO: HBlank interrupt should take priority over VBlank interrupt.
- **Remediation:** Address the comment.

### [F-021] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:277`
- **Description:** // TODO: If emulating SMS1, disable 224-line and 240-line modes.
- **Remediation:** Address the comment.

### [F-022] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:308`
- **Description:** // TODO: Don't rerun the VDP drawing functions when paused!
- **Remediation:** Address the comment.

### [F-023] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:336`
- **Description:** VDP_Lines.NTSC_V30.Offset += 11;	// TODO: Figure out a good offset increment.
- **Remediation:** Address the comment.

### [F-024] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:361`
- **Description:** // TODO: Only update if VDP_Mode is changed.
- **Remediation:** Address the comment.

### [F-025] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:427`
- **Description:** if (VDP_Reg.m5.Set4 & 0x01)	// Check for H40 mode. (TODO: Test 0x81 instead?)
- **Remediation:** Address the comment.

### [F-026] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:443`
- **Description:** if (VDP_Reg.m5.Set4 & 0x01)	// Check for H40 mode. (TODO: Test 0x81 instead?)
- **Remediation:** Address the comment.

### [F-027] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:451`
- **Description:** // TODO: Only set this if the actual value has changed.
- **Remediation:** Address the comment.

### [F-028] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:457`
- **Description:** // TODO: Only set this if the actual value has changed.
- **Remediation:** Address the comment.

### [F-029] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:483`
- **Description:** // TODO: Only set this if the actual value has changed.
- **Remediation:** Address the comment.

### [F-030] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:486`
- **Description:** if (val & 0x81)		// TODO: Original asm tests 0x81. Should this be done for other H40 tests?
- **Remediation:** Address the comment.

### [F-031] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:660`
- **Description:** // TODO: We're checking both RS0 and RS1 here. Others only check one.
- **Remediation:** Address the comment.

### [F-032] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:679`
- **Description:** uint8_t bl, bh;		// TODO: Figure out what this actually means.
- **Remediation:** Address the comment.

### [F-033] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:703`
- **Description:** // TODO: Some of these values are wrong.
- **Remediation:** Address the comment.

### [F-034] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:747`
- **Description:** // Toggle the upper 8 bits of VDP_Status. (TODO: Is this correct?)
- **Remediation:** Address the comment.

### [F-035] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:751`
- **Description:** // TODO: Should these be masked? This might be why some games are broken...
- **Remediation:** Address the comment.

### [F-036] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:775`
- **Description:** // TODO: Test this function.
- **Remediation:** Address the comment.

### [F-037] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:789`
- **Description:** // TODO: Report this as a bug to the gcc developers.
- **Remediation:** Address the comment.

### [F-038] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:838`
- **Description:** // TODO: Use both RS0/RS1, not just RS1.
- **Remediation:** Address the comment.

### [F-039] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:933`
- **Description:** // TODO: This was actually not working in the asm,
- **Remediation:** Address the comment.

### [F-040] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:943`
- **Description:** // TODO: Although we decrement DMAT_Length correctly based on
- **Remediation:** Address the comment.

### [F-041] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1040`
- **Description:** // TODO: According to the Genesis Software Manual, writing at
- **Remediation:** Address the comment.

### [F-042] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1056`
- **Description:** // TODO: The Genesis Software Manual doesn't mention what happens
- **Remediation:** Address the comment.

### [F-043] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1058`
- **Description:** // TODO: VSRam is 80 bytes, but we're allowing a maximum of 128 bytes here...
- **Remediation:** Address the comment.

### [F-044] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1131`
- **Description:** src_address -= 2;	// TODO: What is this for?
- **Remediation:** Address the comment.

### [F-045] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1139`
- **Description:** src_address -= 2;	// TODO: What is this for?
- **Remediation:** Address the comment.

### [F-046] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1190`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-047] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1199`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-048] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1204`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-049] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1209`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-050] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1216`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-051] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1227`
- **Description:** // TODO: The 128 KB wrapping causes garbage on TmEE's mmf.bin (correct),
- **Remediation:** Address the comment.

### [F-052] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1307`
- **Description:** // TODO: Check endianness with regards to the control words. (Wordswapping!)
- **Remediation:** Address the comment.

### [F-053] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1317`
- **Description:** VDP_Ctrl.Access = 5;	// TODO: What does this mean?
- **Remediation:** Address the comment.

### [F-054] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1430`
- **Description:** // TODO: Is this correct with regards to endianness?
- **Remediation:** Address the comment.

### [F-055] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1449`
- **Description:** // TODO: What does this mean?
- **Remediation:** Address the comment.

### [F-056] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1469`
- **Description:** // TODO: This includes invalid addresses!
- **Remediation:** Address the comment.

### [F-057] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1478`
- **Description:** // TODO: This includes invalid addresses!
- **Remediation:** Address the comment.

### [F-058] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1490`
- **Description:** // TODO: Determine how this works.
- **Remediation:** Address the comment.

### [F-059] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1531`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-060] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1536`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-061] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1541`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-062] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1558`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-063] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1563`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-064] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1568`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-065] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1573`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-066] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1578`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-067] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1583`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-068] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1588`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-069] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1593`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-070] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1598`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-071] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1603`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-072] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1608`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-073] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1613`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-074] Test Failure
- **Severity:** High
- **Location:** `None:None`
- **Description:** Run `cargo test` to see failures.
- **Remediation:** Fix failing tests.

### [F-075] Dead Code
- **Severity:** Low
- **Location:** `vdp_io.cpp:None`
- **Description:** File `vdp_io.cpp` appears to be an unused C++ artifact in a Rust project. It contains many TODOs and is not linked.
- **Remediation:** Remove the file.

### [F-076] Operational Risk: Missing System Dependency
- **Severity:** Medium
- **Location:** `Cargo.toml:None`
- **Description:** Runtime tests fail because `libasound2-dev` (ALSA) is required by `cpal` but not documented or present in the environment.
- **Remediation:** Update README.md to list system requirements or make audio optional.

### [F-077] Maintainability: Build Artifacts in Version Control
- **Severity:** Medium
- **Location:** `fuzz/target/:None`
- **Description:** The directory `fuzz/target/` is tracked in git, bloating the repository and causing false positives in analysis.
- **Remediation:** Remove `fuzz/target` from git and add to `.gitignore`.
