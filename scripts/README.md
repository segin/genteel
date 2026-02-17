# Genteel Helper Scripts

This directory contains utility scripts for the Genteel project.

## `audit_tool.py`

A security and quality audit tool that scans the codebase for:
- Potential secrets (API keys, private keys, passwords)
- Technical debt tags (TODO, FIXME, XXX)
- Unsafe code blocks (unsafe { ... })

### Usage

Run from the repository root:

```bash
python3 scripts/audit_tool.py
```

The script will generate reports in the `audit_reports/` directory:
- `findings.json`: JSON format of all findings.
- `RISK_REGISTER.csv`: A CSV file suitable for tracking issues.

## `benchmark_regex.py`

A benchmark script to compare the performance of pre-compiled regex patterns versus inline regex compilation. This is useful for optimizing the audit tool.

### Usage

Run from the repository root:

```bash
python3 scripts/benchmark_regex.py
```
