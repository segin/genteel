import time
import re
import sys
import os
import random
import string
import unittest

# Ensure we can import audit_tool from the scripts directory
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.append(os.path.join(project_root, 'scripts'))
import audit_tool

def generate_data(filename, size_mb):
    print(f"[*] Generating {size_mb}MB test file: {filename}...")
    chunk_size = 1024 * 1024 # 1MB
    chars = string.ascii_letters + string.digits + " \n\t"

    with open(filename, "w", encoding="utf-8") as f:
        for _ in range(size_mb):
            # Generate 1MB of random data
            chunk = "".join(random.choices(chars, k=chunk_size))

            # Inject patterns occasionally
            if random.random() < 0.5:
                chunk += "\nAKIA" + "".join(random.choices(string.ascii_uppercase + string.digits, k=16)) + "\n"
            if random.random() < 0.5:
                chunk += "\n-----BEGIN OPENSSH PRIVATE KEY-----\n"
            if random.random() < 0.5:
                chunk += "\nunsafe {\n"
            if random.random() < 0.5:
                chunk += "\nTODO: fix this\n"

            f.write(chunk)
    print("[*] Generation complete.")

def scan_slow(filename):
    print("[*] Running slow scan (re.search inside loop)...")

    match_count = 0
    start_time = time.time()

    with open(filename, 'r', encoding='utf-8', errors='ignore') as fp:
        for i, line_content in enumerate(fp):
            # Secrets
            for name, pattern in audit_tool.SECRET_PATTERNS.items():
                # Use .pattern to get the raw string from compiled regex
                if re.search(pattern.pattern, line_content):
                    match_count += 1

            # Unsafe
            if re.search(audit_tool.UNSAFE_PATTERN.pattern, line_content):
                match_count += 1

            # TODOs
            if re.search(audit_tool.TODO_PATTERN.pattern, line_content):
                match_count += 1

    end_time = time.time()
    duration = end_time - start_time
    print(f"[*] Slow scan took {duration:.4f} seconds. Matches: {match_count}")
    return duration, match_count

def scan_fast(filename):
    print("[*] Running fast scan (pre-compiled regex)...")

    match_count = 0
    start_time = time.time()

    with open(filename, 'r', encoding='utf-8', errors='ignore') as fp:
        for i, line_content in enumerate(fp):
            # Secrets
            for name, pattern in audit_tool.SECRET_PATTERNS.items():
                if pattern.search(line_content):
                    match_count += 1

            # Unsafe
            if audit_tool.UNSAFE_PATTERN.search(line_content):
                match_count += 1

            # TODOs
            if audit_tool.TODO_PATTERN.search(line_content):
                match_count += 1

    end_time = time.time()
    duration = end_time - start_time
    print(f"[*] Fast scan took {duration:.4f} seconds. Matches: {match_count}")
    return duration, match_count

class TestAuditRegexPerformance(unittest.TestCase):
    def test_regex_performance(self):
        filename = "test_data_small.txt"
        generate_data(filename, 1) # 1MB

        try:
            duration_slow, matches_slow = scan_slow(filename)
            duration_fast, matches_fast = scan_fast(filename)

            # Assert correctness
            self.assertEqual(matches_fast, matches_slow, "Fast scan matches should equal slow scan matches")

            # Performance check (informative)
            print(f"\n[Test Result] Slow: {duration_slow:.4f}s, Fast: {duration_fast:.4f}s")

        finally:
            if os.path.exists(filename):
                os.remove(filename)

if __name__ == "__main__":
    # When run directly, perform the full benchmark
    BENCHMARK_FILE = "benchmark_data_large.txt"
    try:
        if not os.path.exists(BENCHMARK_FILE):
            generate_data(BENCHMARK_FILE, 50)

        t_slow, m_slow = scan_slow(BENCHMARK_FILE)
        t_fast, m_fast = scan_fast(BENCHMARK_FILE)

        print(f"\nResults:")
        print(f"Slow: {t_slow:.4f}s")
        print(f"Fast: {t_fast:.4f}s")
        if t_slow > 0:
            print(f"Improvement: {(t_slow - t_fast) / t_slow * 100:.2f}%")

    finally:
        if os.path.exists(BENCHMARK_FILE):
            os.remove(BENCHMARK_FILE)
