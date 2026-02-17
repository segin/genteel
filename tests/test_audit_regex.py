import unittest
import sys
import os
import time
import random
import string
import re
import tempfile

# Add parent directory to path to import audit_tool
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

import audit_tool

class TestAuditRegex(unittest.TestCase):
    FILE_SIZE_MB = 5  # Smaller size for test (5MB)

    def test_patterns_correctness(self):
        """Verify that regex patterns match expected strings and don't match others."""

        # 1. AWS Key
        aws_pattern = audit_tool.SECRET_PATTERNS["AWS Key"]
        self.assertTrue(aws_pattern.search("AKIAIOSFODNN7EXAMPLE"))
        self.assertFalse(aws_pattern.search("AKIAIOSFODNN7EXAMPL")) # Too short
        self.assertFalse(aws_pattern.search("BKIAIOSFODNN7EXAMPLE")) # Wrong prefix

        # 2. Private Key
        pk_pattern = audit_tool.SECRET_PATTERNS["Private Key"]
        self.assertTrue(pk_pattern.search("-----BEGIN RSA PRIVATE KEY-----"))
        self.assertTrue(pk_pattern.search("-----BEGIN OPENSSH PRIVATE KEY-----"))
        self.assertFalse(pk_pattern.search("-----BEGIN PUBLIC KEY-----"))

        # 3. Generic Token
        token_pattern = audit_tool.SECRET_PATTERNS["Generic Token"]
        self.assertTrue(token_pattern.search("token = 'abcdefghijklmnopqrstuvwxyz0123456789'"))
        self.assertTrue(token_pattern.search('token="ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"'))
        self.assertFalse(token_pattern.search("token = 'short'"))

        # 4. Generic Secret
        secret_pattern = audit_tool.SECRET_PATTERNS["Generic Secret"]
        self.assertTrue(secret_pattern.search("secret = 'my_secret'"))
        self.assertTrue(secret_pattern.search("SECRET: 'my_secret'"))
        self.assertFalse(secret_pattern.search("secret_agent = 'bond'")) # Should not match variable name prefix

        # 5. API Key
        apikey_pattern = audit_tool.SECRET_PATTERNS["API Key"]
        self.assertTrue(apikey_pattern.search("api_key = '12345'"))
        self.assertTrue(apikey_pattern.search("ApiKey: '12345'"))
        self.assertFalse(apikey_pattern.search("api_key_id = '123'")) # Should match? Regex is `api[_-]?key\s*[:=]\s*['\"]`

        # 6. Password
        pwd_pattern = audit_tool.SECRET_PATTERNS["Password"]
        self.assertTrue(pwd_pattern.search("password = 'hunter2'"))
        self.assertTrue(pwd_pattern.search("PASSWORD: 'hunter2'"))
        self.assertFalse(pwd_pattern.search("password_hash = '123'")) # Should match? Regex is `password\s*[:=]\s*['\"]`

        # 7. Unsafe
        unsafe_pattern = audit_tool.UNSAFE_PATTERN
        self.assertTrue(unsafe_pattern.search("unsafe {"))
        self.assertTrue(unsafe_pattern.search("unsafe  {"))
        self.assertFalse(unsafe_pattern.search("safe {"))

        # 8. TODO
        todo_pattern = audit_tool.TODO_PATTERN
        self.assertTrue(todo_pattern.search("TODO: fix this"))
        self.assertTrue(todo_pattern.search("FIXME: urgent"))
        self.assertTrue(todo_pattern.search("XXX: verify"))
        self.assertFalse(todo_pattern.search("NOTE: normal comment"))

    def test_performance(self):
        """Benchmark the regex performance on a generated file."""
        print(f"\n[*] Generating {self.FILE_SIZE_MB}MB test file for performance check...")
        chunk_size = 1024 * 1024 # 1MB
        chars = string.ascii_letters + string.digits + " \n\t"

        with tempfile.NamedTemporaryFile(mode='w+', encoding='utf-8', delete=True) as temp_file:
            for _ in range(self.FILE_SIZE_MB):
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

                temp_file.write(chunk)

            temp_file.flush()
            print("[*] Generation complete. Scanning...")

            start_time = time.time()
            match_count = 0

            # Rewind file to read from beginning
            temp_file.seek(0)

            for line_content in temp_file:
                # Secrets
                for _, pattern in audit_tool.SECRET_PATTERNS.items():
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

            print(f"[*] Scan took {duration:.4f} seconds. Matches: {match_count}")

            # Assert performance is reasonable (e.g., > 10MB/s is a low bar, so 5MB should take < 0.5s)
            self.assertLess(duration, 5.0, f"Scanning {self.FILE_SIZE_MB}MB took too long: {duration:.4f}s")

if __name__ == "__main__":
    unittest.main()
