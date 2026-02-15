# Comprehensive Codebase Audit Findings

**Date:** 2026-02-14T19:29:23.146733
**Commit:** ab0a39b5f8a7e4f5ac6497811e1cc98ccc2014ad

## Metrics
- **Files:** 102
- **Dependencies:** 322
- **Secrets Found:** 1
- **Tests:** Fail

## Detailed Findings
### [F-001] Potential Secret: Private Key
- **Severity:** Critical
- **Location:** `audit_tool.py:174`
- **Description:** Found pattern matching Private Key
- **Remediation:** Rotate secret and remove from history.

### [F-002] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `src/memory/bus.rs:275`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-003] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `src/memory/bus.rs:276`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-004] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `src/memory/bus.rs:365`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-005] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `src/memory/bus.rs:366`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-006] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `src/memory/bus.rs:367`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-007] Unsafe Rust Code
- **Severity:** Medium
- **Location:** `src/memory/bus.rs:368`
- **Description:** Usage of `unsafe` block detected. Verify memory safety manually.
- **Remediation:** Audit unsafe block for soundness.

### [F-008] Test Failure
- **Severity:** High
- **Location:** `None:None`
- **Description:** Run `cargo test` to see failures.
- **Remediation:** Fix failing tests.

