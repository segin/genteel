# Development Scripts

This directory contains utility scripts for the Genteel emulator project.

## `audit_tool.py`

A security and code quality audit tool that scans the codebase for potential secrets, technical debt (TODO/FIXME), and unsafe code blocks.

**Usage:**
Run from the repository root:
```bash
python3 scripts/audit_tool.py
```

The script will generate reports in the `audit_reports/` directory:
- `findings.json`: Detailed list of findings in JSON format.
- `RISK_REGISTER.csv`: A CSV file suitable for tracking issues.

## `benchmark_regex.py`

A utility to benchmark the performance of regex pattern matching used in the audit tool. It generates a large test file and measures the time taken by different scanning approaches.

**Usage:**
Run from the repository root:
```bash
python3 scripts/benchmark_regex.py
```
