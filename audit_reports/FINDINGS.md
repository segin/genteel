# Comprehensive Codebase Audit Findings

**Date:** 2026-02-12T02:34:15.680233
**Commit:** 1a5dbeffc6f165b694ca72fa3a9f56b474e76aa2

## Metrics
- **Files:** 399
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

### [F-015] Test Failure
- **Severity:** High
- **Location:** `None:None`
- **Description:** Run `cargo test` to see failures.
- **Remediation:** Fix failing tests.

