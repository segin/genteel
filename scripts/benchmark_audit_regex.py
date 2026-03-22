import time
import re
import os
import random
import string

FILENAME = "benchmark_data.txt"
FILE_SIZE_MB = 50

def generate_data():
    print(f"[*] Generating {FILE_SIZE_MB}MB test file...")
    chunk_size = 1024 * 1024 # 1MB
    chars = string.ascii_letters + string.digits + " \n\t"

    with open(FILENAME, "w", encoding="utf-8") as f:
        for _ in range(FILE_SIZE_MB):
            # Generate 1MB of random data
            chunk = "".join(random.choices(chars, k=chunk_size))

            # Inject patterns occasionally
            if random.random() < 0.5:
                chunk += "\nAK" + "IA" + "".join(random.choices(string.ascii_uppercase + string.digits, k=16)) + "\n"
            if random.random() < 0.5:
                chunk += "\n-----BEGIN OPENSSH " + "PRIVATE " + "KEY-----\n"
            if random.random() < 0.5:
                chunk += "\nun" + "safe {\n"
            if random.random() < 0.5:
                chunk += "\n" + "TO" + "DO: fix this\n"

            f.write(chunk)
    print("[*] Generation complete.")

def scan_slow():
    print("[*] Running slow scan (re.search inside loop)...")
    secret_pattern_combined = re.compile(
        r"(?P<AWS_Key>AK" + r"IA[0-9A-Z]{16})|"
        r"(?P<Private_Key>-----BEGIN .* PRIVATE" + r" KEY-----)|"
        r"(?P<Generic_Token>token\s*=\s*['\"][a-zA-Z0-9]{20,}['\"])"
    )

    unsafe_pattern = re.compile(r"un" + r"safe\s*\{")
    todo_pattern = re.compile(r"(" + r"TO" + r"DO|" + r"FIX" + r"ME|" + r"X" + r"XX):")

    match_count = 0
    start_time = time.time()

    with open(FILENAME, 'r', encoding='utf-8', errors='ignore') as fp:
        for i, line_content in enumerate(fp):
            # Secrets
            for match in secret_pattern_combined.finditer(line_content):
                match_count += 1

            # Unsafe
            if unsafe_pattern.search(line_content):
                match_count += 1

            # tracker
            if todo_pattern.search(line_content):
                match_count += 1

    end_time = time.time()
    duration = end_time - start_time
    print(f"[*] Slow scan took {duration:.4f} seconds. Matches: {match_count}")
    return duration

def scan_fast():
    print("[*] Running fast scan (pre-compiled regex)...")
    secret_pattern_combined = re.compile(
        r"(?P<AWS_Key>AK" + r"IA[0-9A-Z]{16})|"
        r"(?P<Private_Key>-----BEGIN .* PRIVATE" + r" KEY-----)|"
        r"(?P<Generic_Token>token\s*=\s*['\"][a-zA-Z0-9]{20,}['\"])"
    )

    unsafe_pattern = re.compile(r"un" + r"safe\s*\{")
    todo_pattern = re.compile(r"(" + r"TO" + r"DO|" + r"FIX" + r"ME|" + r"X" + r"XX):")

    match_count = 0
    start_time = time.time()

    with open(FILENAME, 'r', encoding='utf-8', errors='ignore') as fp:
        for i, line_content in enumerate(fp):
            # Secrets
            for match in secret_pattern_combined.finditer(line_content):
                match_count += 1

            # Unsafe
            if unsafe_pattern.search(line_content):
                match_count += 1

            # tracker
            if todo_pattern.search(line_content):
                match_count += 1

    end_time = time.time()
    duration = end_time - start_time
    print(f"[*] Fast scan took {duration:.4f} seconds. Matches: {match_count}")
    return duration

if __name__ == "__main__":
    if not os.path.exists(FILENAME):
        generate_data()

    t_slow = scan_slow()
    t_fast = scan_fast()

    print(f"\nResults:")
    print(f"Slow: {t_slow:.4f}s")
    print(f"Fast: {t_fast:.4f}s")
    print(f"Improvement: {(t_slow - t_fast) / t_slow * 100:.2f}%")

    # Clean up
    if os.path.exists(FILENAME):
        os.remove(FILENAME)
