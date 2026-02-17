# Developer Scripts

This directory contains utility scripts for development, testing, and auditing of the Genteel codebase.

## Scripts

### `audit_tool.py`

A security and quality audit tool that scans the codebase for:
*   Potential secrets (API keys, tokens, etc.)
*   Technical debt markers (TODO, FIXME, XXX)
*   Unsafe code blocks (`unsafe { ... }`)

**Usage:**

Run this script from the **repository root**:

```bash
python3 scripts/audit_tool.py
```

The script will generate an `audit_reports/` directory in the root containing:
*   `findings.json`: A JSON list of all findings.
*   `RISK_REGISTER.csv`: A CSV file suitable for tracking issues.

### `benchmark_regex.py`

A benchmarking script used to compare the performance of regex scanning methods (e.g., pre-compiled vs. ad-hoc regex). This script is useful for verifying the efficiency of patterns used in the audit tool or other parts of the system.

**Usage:**

Run this script from the **repository root**:

```bash
python3 scripts/benchmark_regex.py
```
