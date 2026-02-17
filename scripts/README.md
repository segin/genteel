# Development Scripts

This directory contains utility scripts for the Genteel project.

## Audit Tool (`audit_tool.py`)

A security and quality audit tool that scans the codebase for:
- Potential secrets (API keys, passwords, etc.)
- Technical debt (TODO, FIXME, XXX)
- Unsafe code blocks (Rust `unsafe`)

**Usage:**
Run from the repository root:
```bash
python3 scripts/audit_tool.py
```
Reports are generated in the `audit_reports/` directory.

## Benchmark Regex (`benchmark_regex.py`)

A utility to benchmark regex performance, comparing `re.search` inside a loop vs. pre-compiled regex.

**Usage:**
Run from the repository root:
```bash
python3 scripts/benchmark_regex.py
```
