#!/usr/bin/env python3
import os
import re
import json
import csv
import subprocess
from datetime import datetime

# =============================================================================
# Security & Quality Audit Tool for genteel
# =============================================================================

# Calculate repository root (assumes this script is in scripts/ directory)
REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

REPORT_DIR = os.path.join(REPO_ROOT, "audit_reports")
FINDINGS_JSON = os.path.join(REPORT_DIR, "findings.json")
FINDINGS_MD = os.path.join(REPORT_DIR, "FINDINGS.md")
METRICS_JSON = os.path.join(REPORT_DIR, "metrics.json")
RISK_CSV = os.path.join(REPORT_DIR, "RISK_REGISTER.csv")

findings = []

# Pre-compiled regex patterns at global scope for performance
SECRET_PATTERNS = {
    "Generic Secret": re.compile(r"(?i)secret\s*[:=]\s*['\"]"),
    "API Key": re.compile(r"(?i)api[_-]?key\s*[:=]\s*['\"]"),
    "Password": re.compile(r"(?i)password\s*[:=]\s*['\"]"),
    "AWS Key": re.compile(r"AKIA[0-9A-Z]{16}"),
    "Private Key": re.compile(r"-----BEGIN .* PRIVATE KEY-----"),
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
        # Run git ls-files from the repository root
        out = subprocess.check_output(["git", "ls-files"], cwd=REPO_ROOT, stderr=subprocess.STDOUT).decode("utf-8")
        files = out.splitlines()
        # Filter out target directories and audit reports
        return [f for f in files if not f.startswith("audit_reports/") and "/target/" not in f and not f.startswith("target/")]
    except:
        # Fallback to manual scan if git fails
        files = []
        for root, _, filenames in os.walk(REPO_ROOT):
            if ".git" in root or "target" in root or "audit_reports" in root:
                continue
            for f in filenames:
                if f.endswith((".rs", ".py", ".md", ".sh", ".toml")):
                    # Store path relative to REPO_ROOT for consistency
                    full_path = os.path.join(root, f)
                    rel_path = os.path.relpath(full_path, REPO_ROOT)
                    files.append(rel_path)
        return files

def scan_text_patterns():
    files = get_tracked_files()

    for f in files:
        full_path = os.path.join(REPO_ROOT, f)
        if not os.path.exists(full_path) or os.path.isdir(full_path):
            continue
        if not f.endswith((".rs", ".py", ".md", ".sh", ".toml")):
            continue

        try:
            with open(full_path, 'r', encoding='utf-8', errors='ignore') as fp:
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
    print(f"🚀 Starting genteel security & quality audit from {REPO_ROOT}...")
    
    if not os.path.exists(REPORT_DIR):
        os.makedirs(REPORT_DIR)

    scan_text_patterns()
    
    # Save Findings
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
