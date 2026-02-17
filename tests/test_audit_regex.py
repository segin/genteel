import unittest
import sys
import os
import re
import random
import string
import time

# Add parent directory to sys.path to import audit_tool
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

import audit_tool

class TestAuditRegex(unittest.TestCase):
    def test_secret_patterns(self):
        # Test positive matches
        test_cases = [
            ("AWS Key", "AKIAABCD1234567890XY"),
            ("Private Key", "-----BEGIN OPENSSH PRIVATE KEY-----"),
            ("Generic Token", 'token = "abcdefghijklmnopqrstuvwxyz123456"'),
            ("Generic Secret", "secret = 'hidden_value'"),
            ("API Key", "api_key = '12345-abcde'"),
            ("Password", "password = 'secret_password'")
        ]

        for pattern_name, text in test_cases:
            pattern = audit_tool.SECRET_PATTERNS.get(pattern_name)
            self.assertIsNotNone(pattern, f"Pattern {pattern_name} not found")
            self.assertTrue(pattern.search(text), f"Pattern {pattern_name} failed to match '{text}'")

    def test_todo_pattern(self):
        self.assertTrue(audit_tool.TODO_PATTERN.search("TODO: Implement this"))
        self.assertTrue(audit_tool.TODO_PATTERN.search("FIXME: Broken logic"))
        self.assertTrue(audit_tool.TODO_PATTERN.search("XXX: Hacky solution"))
        self.assertFalse(audit_tool.TODO_PATTERN.search("This is done."))

    def test_unsafe_pattern(self):
        self.assertTrue(audit_tool.UNSAFE_PATTERN.search("unsafe {"))
        self.assertTrue(audit_tool.UNSAFE_PATTERN.search("unsafe  {"))
        self.assertFalse(audit_tool.UNSAFE_PATTERN.search("safe {"))

    def test_performance_sanity(self):
        # Generate 1MB of random data with occasional patterns
        # Using a smaller size than the original 50MB benchmark for speed
        chunk_size = 1024 * 1024
        chars = string.ascii_letters + string.digits + " \n\t"

        # Use a fixed seed for reproducibility
        random.seed(42)

        data = "".join(random.choices(chars, k=chunk_size))

        # Inject patterns
        data += "\nAKIA" + "".join(random.choices(string.ascii_uppercase + string.digits, k=16)) + "\n"
        data += "\nTODO: fix this\n"
        data += "\nunsafe {\n"

        start_time = time.time()

        # Simulate scanning
        match_count = 0
        lines = data.splitlines()
        for line in lines:
            for pattern in audit_tool.SECRET_PATTERNS.values():
                if pattern.search(line):
                    match_count += 1
            if audit_tool.TODO_PATTERN.search(line):
                match_count += 1
            if audit_tool.UNSAFE_PATTERN.search(line):
                match_count += 1

        end_time = time.time()
        duration = end_time - start_time

        # Ensure it's reasonably fast (1MB should take negligible time)
        # We set a generous upper bound to avoid flakiness on slow CI
        self.assertLess(duration, 1.0, f"Scanning 1MB took too long: {duration:.4f}s")
        self.assertGreater(match_count, 0, "Should have found matches")

if __name__ == '__main__':
    unittest.main()
