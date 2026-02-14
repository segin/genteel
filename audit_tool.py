import os
import subprocess
import json
import re
import tomllib
import datetime
import csv
from pathlib import Path

# Configuration
AUDIT_DIR = "audit_reports"
REMEDIATION_DIR = os.path.join(AUDIT_DIR, "remediations")
FINDINGS_FILE = os.path.join(AUDIT_DIR, "findings.json")
SUMMARY_FILE = os.path.join(AUDIT_DIR, "EXECUTIVE_SUMMARY.md")
FULL_REPORT_FILE = os.path.join(AUDIT_DIR, "FINDINGS.md")
RISK_REGISTER_FILE = os.path.join(AUDIT_DIR, "RISK_REGISTER.csv")
METRICS_FILE = os.path.join(AUDIT_DIR, "metrics.json")

# Ensure directories exist
os.makedirs(REMEDIATION_DIR, exist_ok=True)

findings = []
metrics = {
    "loc": {},
    "file_count": 0,
    "languages": set(),
    "dependency_count": 0,
    "vulnerabilities": 0,
    "secrets_found": 0,
    "complexity_hotspots": [],
    "test_results": "Not Run"
}

TRACKED_FILES = None

def run_command(command, cwd=None, capture_output=True):
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            capture_output=capture_output,
            text=True,
            check=False
        )
        return result.stdout, result.stderr, result.returncode
    except Exception as e:
        return "", str(e), -1

def add_finding(title, severity, description, filepath=None, line=None, remediation=None):
    finding = {
        "id": f"F-{len(findings)+1:03d}",
        "title": title,
        "severity": severity, # Critical, High, Medium, Low, Info
        "description": description,
        "file": filepath,
        "line": line,
        "remediation": remediation
    }
    findings.append(finding)

def get_tracked_files():
    global TRACKED_FILES
    if TRACKED_FILES is not None:
        return TRACKED_FILES

    files = []
    # Use git ls-files to get tracked files
    out, err, _ = run_command(["git", "ls-files"])
    if out:
        files = out.splitlines()
    else:
        # Fallback to os.walk
        for root, _, filenames in os.walk("."):
            if ".git" in root: continue
            if "audit_reports" in root: continue
            for f in filenames:
                files.append(os.path.join(root, f))

    # Filter out audit_reports from files if git ls-files included them (unlikely if not committed yet, but possible)
    files = [f for f in files if not f.startswith("audit_reports/") and "/target/" not in f and not f.startswith("target/")]

    TRACKED_FILES = files
    return files

def enumerate_files():
    print("[*] Enumerating files...")
    files = get_tracked_files()

    metrics["file_count"] = len(files)

    # LOC and Language detection
    loc_by_lang = {}
    for f in files:
        ext = os.path.splitext(f)[1]
        lang = "Unknown"
        if ext == ".rs": lang = "Rust"
        elif ext in [".c", ".cpp", ".h", ".hpp"]: lang = "C/C++"
        elif ext == ".py": lang = "Python"
        elif ext == ".md": lang = "Markdown"
        elif ext == ".toml": lang = "TOML"
        elif ext == ".json": lang = "JSON"

        metrics["languages"].add(lang)

        try:
            with open(f, 'r', encoding='utf-8', errors='ignore') as fp:
                lines = sum(1 for _ in fp)
                loc_by_lang[lang] = loc_by_lang.get(lang, 0) + lines
        except:
            pass

    metrics["loc"] = loc_by_lang

def run_sast_clippy():
    print("[*] Running Cargo Clippy...")
    # cargo clippy --message-format=json
    out, err, _ = run_command(["cargo", "clippy", "--all-targets", "--all-features", "--message-format=json"])

    for line in out.splitlines():
        try:
            msg = json.loads(line)
            if msg.get("reason") == "compiler-message":
                message = msg.get("message", {})
                level = message.get("level")
                if level in ["error", "warning"]:
                    code = message.get("code", {}).get("code", "unknown")
                    text = message.get("message")
                    spans = message.get("spans", [])

                    file_path = "unknown"
                    line_num = 0
                    if spans:
                        file_path = spans[0].get("file_name")
                        line_num = spans[0].get("line_start")

                    severity = "High" if level == "error" else "Medium"

                    add_finding(
                        title=f"Clippy: {code}",
                        severity=severity,
                        description=text,
                        filepath=file_path,
                        line=line_num,
                        remediation="Apply clippy suggestion or refactor."
                    )
        except:
            pass

def run_sast_cppcheck():
    print("[*] Running Cppcheck...")
    # specific for vdp_io.cpp or all cpp files
    files = get_tracked_files()
    cpp_files = [f for f in files if f.endswith(".cpp") or f.endswith(".c")]

    for f in cpp_files:
        out, err, _ = run_command(["cppcheck", "--enable=all", f])
        if err: # cppcheck prints to stderr
            for line in err.splitlines():
                if "error" in line or "warning" in line:
                    add_finding(
                        title="Cppcheck Finding",
                        severity="Medium",
                        description=line,
                        filepath=f,
                        remediation="Fix C++ issue."
                    )

def scan_text_patterns():
    print("[*] Scanning for secrets and unsafe patterns...")
    files = get_tracked_files()

    secret_patterns = {
        "AWS Key": re.compile(r"AKIA[0-9A-Z]{16}"),
        "Private Key": re.compile(r"-----BEGIN .* PRIVATE KEY-----"),
        "Generic Token": re.compile(r"token\s*=\s*['\"][a-zA-Z0-9]{20,}['\"]"),
    }

    unsafe_pattern = re.compile(r"unsafe\s*\{")
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
                                filepath=f,
                                line=i+1,
                                remediation="Rotate secret and remove from history."
                            )
                            metrics["secrets_found"] += 1

                    # Unsafe
                    if unsafe_pattern.search(line_content):
                        add_finding(
                            title="Unsafe Rust Code",
                            severity="Medium",
                            description="Usage of `unsafe` block detected. Verify memory safety manually.",
                            filepath=f,
                            line=i+1,
                            remediation="Audit unsafe block for soundness."
                        )

                    # TODOs
                    if todo_pattern.search(line_content):
                        add_finding(
                            title="Technical Debt (TODO/FIXME)",
                            severity="Info",
                            description=line_content.strip(),
                            filepath=f,
                            line=i+1,
                            remediation="Address the comment."
                        )

        except Exception as e:
            pass

def analyze_dependencies():
    print("[*] Analyzing dependencies...")
    if os.path.exists("Cargo.lock"):
        try:
            with open("Cargo.lock", "rb") as f:
                data = tomllib.load(f)

            packages = data.get("package", [])
            metrics["dependency_count"] = len(packages)

            for pkg in packages:
                name = pkg.get("name")
                version = pkg.get("version")
                # source = pkg.get("source", "local")

                # Manual vulnerability check placeholder
                # In a real agent with net access, we'd query OSV or RustSec
                # For now we just log it.
                pass

        except Exception as e:
            add_finding("Dependency Parse Error", "Low", f"Failed to parse Cargo.lock: {str(e)}")

def run_tests():
    print("[*] Running tests...")
    out, err, code = run_command(["cargo", "test", "--no-fail-fast"])

    with open(os.path.join(AUDIT_DIR, "test_output.txt"), "w") as f:
        f.write(out + "\n" + err)

    metrics["test_results"] = "Pass" if code == 0 else "Fail"
    if code != 0:
        add_finding(
            title="Test Failure",
            severity="High",
            description="Run `cargo test` to see failures.",
            remediation="Fix failing tests."
        )

def generate_reports():
    print("[*] Generating reports...")

    # metrics.json
    metrics["languages"] = list(metrics["languages"])
    with open(METRICS_FILE, "w") as f:
        json.dump(metrics, f, indent=2)

    # findings.json
    with open(FINDINGS_FILE, "w") as f:
        json.dump(findings, f, indent=2)

    # RISK_REGISTER.csv
    with open(RISK_REGISTER_FILE, "w", newline='') as f:
        writer = csv.writer(f)
        writer.writerow(["ID", "Title", "Severity", "File", "Line", "Status"])
        for finding in findings:
            writer.writerow([
                finding["id"],
                finding["title"],
                finding["severity"],
                finding.get("file", ""),
                finding.get("line", ""),
                "Open"
            ])

    # FINDINGS.md
    with open(FULL_REPORT_FILE, "w") as f:
        f.write("# Comprehensive Codebase Audit Findings\n\n")
        f.write(f"**Date:** {datetime.datetime.now().isoformat()}\n")
        f.write(f"**Commit:** {run_command(['git', 'rev-parse', 'HEAD'])[0].strip()}\n\n")

        f.write("## Metrics\n")
        f.write(f"- **Files:** {metrics['file_count']}\n")
        f.write(f"- **Dependencies:** {metrics['dependency_count']}\n")
        f.write(f"- **Secrets Found:** {metrics['secrets_found']}\n")
        f.write(f"- **Tests:** {metrics['test_results']}\n\n")

        f.write("## Detailed Findings\n")
        for finding in findings:
            f.write(f"### [{finding['id']}] {finding['title']}\n")
            f.write(f"- **Severity:** {finding['severity']}\n")
            f.write(f"- **Location:** `{finding.get('file', 'N/A')}:{finding.get('line', 'N/A')}`\n")
            f.write(f"- **Description:** {finding['description']}\n")
            f.write(f"- **Remediation:** {finding.get('remediation', 'N/A')}\n\n")

    # EXECUTIVE_SUMMARY.md (Skeleton, to be refined manually)
    with open(SUMMARY_FILE, "w") as f:
        f.write("# Executive Summary\n\n")
        f.write("## Overview\n")
        f.write("This audit assessed the security, correctness, and maintainability of the `genteel` codebase.\n\n")
        f.write("## Key Risks\n")
        # List top 5 high/critical findings
        high_risks = [x for x in findings if x['severity'] in ['Critical', 'High']]
        for risk in high_risks[:5]:
            f.write(f"- **{risk['title']}**: {risk['description'][:100]}...\n")

        if not high_risks:
            f.write("- No Critical or High risks detected automatically.\n")

        f.write("\n## Health Score\n")
        f.write("To be calculated based on manual review.\n")

if __name__ == "__main__":
    enumerate_files()
    run_sast_clippy()
    run_sast_cppcheck()
    scan_text_patterns()
    analyze_dependencies()
    run_tests()
    generate_reports()
    print("[*] Audit complete. Reports generated in " + AUDIT_DIR)
