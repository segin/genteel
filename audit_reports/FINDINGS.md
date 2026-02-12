# Comprehensive Codebase Audit Findings

**Date:** 2026-02-12T02:23:41.888521
**Commit:** 1a5dbeffc6f165b694ca72fa3a9f56b474e76aa2

## Metrics
- **Files:** 400
- **Dependencies:** 322
- **Secrets Found:** 1
- **Tests:** Fail

## Detailed Findings
### [F-001] Potential Secret: Private Key
- **Severity:** Critical
- **Location:** `audit_tool.py:167`
- **Description:** Found pattern matching Private Key
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
- **Location:** `src/apu/ym2612.rs:72`
- **Description:** // TODO: Proper timer implementation.
- **Remediation:** Address the comment.

### [F-013] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `src/main.rs:228`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-014] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `src/main.rs:240`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-015] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:48`
- **Description:** // Miscellaneous. (TODO: Determine if these should be signed or unsigned.)
- **Remediation:** Address the comment.

### [F-016] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:224`
- **Description:** // TODO: HBlank interrupt should take priority over VBlank interrupt.
- **Remediation:** Address the comment.

### [F-017] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:248`
- **Description:** // TODO: Don't rerun the VDP drawing functions when paused!
- **Remediation:** Address the comment.

### [F-018] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:281`
- **Description:** // TODO: If emulating SMS1, disable 224-line and 240-line modes.
- **Remediation:** Address the comment.

### [F-019] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:339`
- **Description:** VDP_Lines.NTSC_V30.Offset += 11;	// TODO: Figure out a good offset increment.
- **Remediation:** Address the comment.

### [F-020] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:364`
- **Description:** // TODO: Only update if VDP_Mode is changed.
- **Remediation:** Address the comment.

### [F-021] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:430`
- **Description:** if (VDP_Reg.m5.Set4 & 0x01)	// Check for H40 mode. (TODO: Test 0x81 instead?)
- **Remediation:** Address the comment.

### [F-022] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:446`
- **Description:** if (VDP_Reg.m5.Set4 & 0x01)	// Check for H40 mode. (TODO: Test 0x81 instead?)
- **Remediation:** Address the comment.

### [F-023] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:454`
- **Description:** // TODO: Only set this if the actual value has changed.
- **Remediation:** Address the comment.

### [F-024] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:460`
- **Description:** // TODO: Only set this if the actual value has changed.
- **Remediation:** Address the comment.

### [F-025] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:486`
- **Description:** // TODO: Only set this if the actual value has changed.
- **Remediation:** Address the comment.

### [F-026] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:489`
- **Description:** if (val & 0x81)		// TODO: Original asm tests 0x81. Should this be done for other H40 tests?
- **Remediation:** Address the comment.

### [F-027] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:663`
- **Description:** // TODO: We're checking both RS0 and RS1 here. Others only check one.
- **Remediation:** Address the comment.

### [F-028] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:682`
- **Description:** uint8_t bl, bh;		// TODO: Figure out what this actually means.
- **Remediation:** Address the comment.

### [F-029] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:706`
- **Description:** // TODO: Some of these values are wrong.
- **Remediation:** Address the comment.

### [F-030] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:750`
- **Description:** // Toggle the upper 8 bits of VDP_Status. (TODO: Is this correct?)
- **Remediation:** Address the comment.

### [F-031] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:754`
- **Description:** // TODO: Should these be masked? This might be why some games are broken...
- **Remediation:** Address the comment.

### [F-032] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:778`
- **Description:** // TODO: Test this function.
- **Remediation:** Address the comment.

### [F-033] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:792`
- **Description:** // TODO: Report this as a bug to the gcc developers.
- **Remediation:** Address the comment.

### [F-034] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:841`
- **Description:** // TODO: Use both RS0/RS1, not just RS1.
- **Remediation:** Address the comment.

### [F-035] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:936`
- **Description:** // TODO: This was actually not working in the asm,
- **Remediation:** Address the comment.

### [F-036] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:946`
- **Description:** // TODO: Although we decrement DMAT_Length correctly based on
- **Remediation:** Address the comment.

### [F-037] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1043`
- **Description:** // TODO: According to the Genesis Software Manual, writing at
- **Remediation:** Address the comment.

### [F-038] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1059`
- **Description:** // TODO: The Genesis Software Manual doesn't mention what happens
- **Remediation:** Address the comment.

### [F-039] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1061`
- **Description:** // TODO: VSRam is 80 bytes, but we're allowing a maximum of 128 bytes here...
- **Remediation:** Address the comment.

### [F-040] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1134`
- **Description:** src_address -= 2;	// TODO: What is this for?
- **Remediation:** Address the comment.

### [F-041] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1142`
- **Description:** src_address -= 2;	// TODO: What is this for?
- **Remediation:** Address the comment.

### [F-042] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1193`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-043] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1202`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-044] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1207`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-045] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1212`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-046] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1219`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-047] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1230`
- **Description:** // TODO: The 128 KB wrapping causes garbage on TmEE's mmf.bin (correct),
- **Remediation:** Address the comment.

### [F-048] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1310`
- **Description:** // TODO: Check endianness with regards to the control words. (Wordswapping!)
- **Remediation:** Address the comment.

### [F-049] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1320`
- **Description:** VDP_Ctrl.Access = 5;	// TODO: What does this mean?
- **Remediation:** Address the comment.

### [F-050] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1433`
- **Description:** // TODO: Is this correct with regards to endianness?
- **Remediation:** Address the comment.

### [F-051] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1452`
- **Description:** // TODO: What does this mean?
- **Remediation:** Address the comment.

### [F-052] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1472`
- **Description:** // TODO: This includes invalid addresses!
- **Remediation:** Address the comment.

### [F-053] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1481`
- **Description:** // TODO: This includes invalid addresses!
- **Remediation:** Address the comment.

### [F-054] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1493`
- **Description:** // TODO: Determine how this works.
- **Remediation:** Address the comment.

### [F-055] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1534`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-056] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1539`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-057] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1544`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-058] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1561`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-059] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1566`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-060] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1571`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-061] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1576`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-062] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1581`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-063] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1586`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-064] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1591`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-065] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1596`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-066] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1601`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-067] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1606`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-068] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1611`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-069] Technical Debt (TODO/FIXME)
- **Severity:** Info
- **Location:** `vdp_io.cpp:1616`
- **Description:** // TODO: This is untested!
- **Remediation:** Address the comment.

### [F-070] Test Failure
- **Severity:** High
- **Location:** `None:None`
- **Description:** Run `cargo test` to see failures.
- **Remediation:** Fix failing tests.

