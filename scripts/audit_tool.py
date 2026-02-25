#!/usr/bin/env python3
"""
Security & Quality Audit Tool for genteel.

This script scans the repository for potential secrets, TODO items, and unsafe code blocks.
It generates a JSON report and a CSV risk register.

Usage:
    Run from anywhere within the repository:
    $ python3 scripts/audit_tool.py

    The report will be generated in the `audit_reports/` directory at the project root.
"""

import os
import sys
import re
import json
import csv
import subprocess
import argparse
from datetime import datetime

# =============================================================================
# Security & Quality Audit Tool for genteel
# =============================================================================

def find_project_root():
    """
    Finds the project root directory by looking for .git or Cargo.toml.
    Returns the path to the root directory.
    """
    current_dir = os.path.abspath(os.path.dirname(__file__))
    while True:
        if os.path.exists(os.path.join(current_dir, ".git")) or os.path.exists(os.path.join(current_dir, "Cargo.toml")):
            return current_dir
        parent_dir = os.path.dirname(current_dir)
        if parent_dir == current_dir:
            return None
        current_dir = parent_dir

# Change to project root to ensure consistent paths
project_root = find_project_root()
if project_root:
    os.chdir(project_root)
else:
    print("Error: Could not find project root (looking for .git or Cargo.toml)")
    sys.exit(1)

REPORT_DIR = "audit_reports"
FINDINGS_JSON = os.path.join(REPORT_DIR, "findings.json")
RISK_CSV = os.path.join(REPORT_DIR, "RISK_REGISTER.csv")

findings = []

# Pre-compiled regex patterns at global scope for performance.
# Note: String concatenation is used for some patterns (e.g., Private Key) to prevent this script from detecting itself as a false positive.
SECRET_PATTERNS = {
    "Generic Secret": re.compile(r"(?i)secret\s*[:=]\s*['\"]"),
    "API Key": re.compile(r"(?i)api[_-]?key\s*[:=]\s*['\"]"),
    "Password": re.compile(r"(?i)password\s*[:=]\s*['\"]"),
    "AWS Key": re.compile(r"AKIA[0-9A-Z]{16}"),
    # Split string to avoid self-flagging (the pattern itself matches the source code string otherwise)
    "Private Key": re.compile(r"-----BEGIN .* PRIVATE " + r"KEY-----"),
    "Generic Token": re.compile(r"token\s*=\s*['\"][a-zA-Z0-9]{20,}['\"]")
}

TODO_PATTERN = re.compile(r"(TODO|FIXME|XXX):")
UNSAFE_PATTERN = re.compile(r"unsafe\s*\{")

def add_finding(title, severity, description, file_path, line_number=None):
    findings.append({
        "title": title,
        "severity": severity,
        "description": description,
        "file": file_path,
        "line": line_number,
        "timestamp": datetime.now().isoformat()
    })

def get_tracked_files():
    try:
        # Run git ls-files to get all tracked files
        out = subprocess.check_output(["git", "ls-files"], stderr=subprocess.STDOUT).decode("utf-8")
        files = out.splitlines()
        # Filter out target directories and audit reports
        return [f for f in files if not f.startswith("audit_reports/") and "/target/" not in f and not f.startswith("target/")]
    except Exception:
        # Fallback to manual scan if git fails
        files = []
        for root, _, filenames in os.walk("."):
            if ".git" in root or "target" in root or "audit_reports" in root:
                continue
            for f in filenames:
                if f.endswith((".rs", ".py", ".md", ".sh", ".toml")):
                    files.append(os.path.relpath(os.path.join(root, f), "."))
        return files

def scan_text_patterns():
    files = get_tracked_files()

    for f in files:
        if not os.path.exists(f) or os.path.isdir(f):
            continue
        if not f.endswith((".rs", ".py", ".md", ".sh", ".toml")):
            continue

        # Skip this script and the benchmark script as they contain the patterns themselves
        if f.endswith("audit_tool.py") or f.endswith("benchmark_regex.py"):
            continue

        try:
            with open(f, 'r', encoding='utf-8', errors='ignore') as fp:
                for i, line_content in enumerate(fp):
                    # Secrets
                    for name, compiled_pattern in SECRET_PATTERNS.items():
                        if compiled_pattern.search(line_content):
                            add_finding(
                                title=f"Potential Secret: {name}",
                                severity="Critical",
                                description=f"Found pattern matching {name}",
                                file_path=f,
                                line_number=i+1
                            )
                    
                    # Technical Debt
                    if TODO_PATTERN.search(line_content):
                        add_finding(
                            title="Technical Debt",
                            severity="Low",
                            description="Unresolved TODO/FIXME/XXX tag",
                            file_path=f,
                            line_number=i+1
                        )

                    # Unsafe Code
                    if UNSAFE_PATTERN.search(line_content):
                        add_finding(
                            title="Unsafe Code",
                            severity="Medium",
                            description="Manual audit required for unsafe block",
                            file_path=f,
                            line_number=i+1
                        )
        except Exception as e:
            print(f"Error scanning {f}: {e}")

def run_audit():
    parser = argparse.ArgumentParser(description="Security & Quality Audit Tool for genteel")
    parser.parse_args()

    # Ensure we are in root
    if not os.path.exists("Cargo.toml"):
        print("Error: Must run from repository root containing Cargo.toml")
        sys.exit(1)

    print("🚀 Starting genteel security & quality audit...")
    print(f"📂 Project root: {os.getcwd()}")
    
    if not os.path.exists(REPORT_DIR):
        os.makedirs(REPORT_DIR)

    scan_text_patterns()
    
    # Save Findings (JSON)
    with open(FINDINGS_JSON, 'w') as f:
        json.dump(findings, f, indent=2)
    
    # Save Risk Register (CSV)
    with open(RISK_CSV, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(["ID", "Title", "Severity", "File", "Line", "Status"])
        for i, finding in enumerate(findings):
            writer.writerow([
                f"AUDIT-{i+1:03}",
                finding["title"],
                finding["severity"],
                finding["file"],
                finding.get("line", "N/A"),
                "Open"
            ])

    print(f"✅ Audit complete! Found {len(findings)} issues.")
    print(f"📄 Reports available in {REPORT_DIR}/")

if __name__ == "__main__":
    run_audit()
