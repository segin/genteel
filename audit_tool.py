#!/usr/bin/env python3
import os
import re
import json
import csv
from datetime import datetime

# =============================================================================
# Security & Quality Audit Tool for genteel
# =============================================================================

REPORT_DIR = "audit_reports"
FINDINGS_JSON = os.path.join(REPORT_DIR, "findings.json")
FINDINGS_MD = os.path.join(REPORT_DIR, "FINDINGS.md")
METRICS_JSON = os.path.join(REPORT_DIR, "metrics.json")
RISK_CSV = os.path.join(REPORT_DIR, "RISK_REGISTER.csv")

findings = []

def add_finding(title, severity, description, file_path, line_number=None):
    findings.append({
        "title": title,
        "severity": severity,
        "description": description,
        "file": file_path,
        "line": line_number,
        "timestamp": datetime.now().isoformat()
    })

def scan_text_patterns():
    # Pre-compiled regex patterns for performance
    secret_patterns = {
        "Generic Secret": re.compile(r"(?i)secret\s*[:=]\s*['\"]"),
        "API Key": re.compile(r"(?i)api[_-]?key\s*[:=]\s*['\"]"),
        "Password": re.compile(r"(?i)password\s*[:=]\s*['\"]")
    }
    
    files = []
    for root, _, filenames in os.walk("."):
        if ".git" in root or "target" in root: continue
        for f in filenames:
            if f.endswith((".rs", ".py", ".md", ".sh", ".toml")):
                files.append(os.path.join(root, f))

    todo_pattern = re.compile(r"(TODO|FIXME|XXX):")

    for f in files:
        if not os.path.exists(f): continue
        if os.path.isdir(f): continue

        try:
            with open(f, 'r', encoding='utf-8', errors='ignore') as fp:
                for i, line_content in enumerate(fp):
                    # Secrets
                    for name, compiled_pattern in secret_patterns.items():
                        if compiled_pattern.search(line_content):
                            add_finding(
                                title=f"Potential Secret: {name}",
                                severity="Critical",
                                description=f"Found pattern matching {name}",
                                file_path=f,
                                line_number=i+1
                            )
                    
                    # Technical Debt
                    if todo_pattern.search(line_content):
                        add_finding(
                            title="Technical Debt",
                            severity="Low",
                            description="Unresolved TODO/FIXME/XXX tag",
                            file_path=f,
                            line_number=i+1
                        )
        except Exception as e:
            print(f"Error scanning {f}: {e}")

def run_audit():
    print("🚀 Starting genteel security & quality audit...")
    
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
