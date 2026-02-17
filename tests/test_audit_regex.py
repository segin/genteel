import unittest
import sys
import os
import re
import time

# Add repo root to path so we can import audit_tool
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import audit_tool

class TestAuditRegex(unittest.TestCase):
    def test_secret_patterns(self):
        patterns = audit_tool.SECRET_PATTERNS

        # Test AWS Key
        self.assertTrue(patterns["AWS Key"].search("AKIA1234567890ABCDEF"))
        self.assertFalse(patterns["AWS Key"].search("AKIB1234567890ABCDEF")) # Wrong prefix

        # Test Private Key
        self.assertTrue(patterns["Private Key"].search("-----BEGIN RSA PRIVATE KEY-----"))
        self.assertFalse(patterns["Private Key"].search("-----BEGIN PUBLIC KEY-----"))

        # Test Generic Token
        self.assertTrue(patterns["Generic Token"].search("token = 'abcdef1234567890abcdef'"))
        self.assertFalse(patterns["Generic Token"].search("token = 'short'"))

        # Test Generic Secret
        self.assertTrue(patterns["Generic Secret"].search("secret = '123'"))
        self.assertTrue(patterns["Generic Secret"].search("SECRET : '123'"))
        self.assertFalse(patterns["Generic Secret"].search("secretary = '123'")) # Should not match secretary

        # Test API Key
        self.assertTrue(patterns["API Key"].search("api_key = 'abc'"))
        self.assertTrue(patterns["API Key"].search("ApiKey: 'abc'"))

        # Test Password
        self.assertTrue(patterns["Password"].search("password = '123'"))
        self.assertTrue(patterns["Password"].search("PASSWORD: '123'"))

    def test_todo_pattern(self):
        pattern = audit_tool.TODO_PATTERN
        self.assertTrue(pattern.search("TODO: fix this"))
        self.assertTrue(pattern.search("FIXME: urgent"))
        self.assertTrue(pattern.search("XXX: verify"))
        self.assertFalse(pattern.search("todo normal text")) # Case sensitive based on regex definition

    def test_unsafe_pattern(self):
        pattern = audit_tool.UNSAFE_PATTERN
        self.assertTrue(pattern.search("unsafe {"))
        self.assertTrue(pattern.search("unsafe  {"))
        self.assertFalse(pattern.search("unsafe"))

    def test_performance_sanity(self):
        # Generate ~1MB of data
        # 100 chars * 10000 lines = 1,000,000 chars approx 1MB
        lines = ["a" * 100 for _ in range(10000)]
        lines.append("AKIA1234567890ABCDEF")
        lines.append("TODO: fix this")
        lines.append("unsafe {")

        start_time = time.time()

        match_count = 0
        for line in lines:
            # Secrets
            for name, pattern in audit_tool.SECRET_PATTERNS.items():
                if pattern.search(line):
                    match_count += 1

            # TODOs
            if audit_tool.TODO_PATTERN.search(line):
                match_count += 1

            # Unsafe
            if audit_tool.UNSAFE_PATTERN.search(line):
                match_count += 1

        duration = time.time() - start_time

        # We expect at least 3 matches (AWS Key, TODO, Unsafe)
        # Note: AWS Key pattern matches "AKIA..." which is present.
        self.assertGreaterEqual(match_count, 3)
        self.assertLess(duration, 1.0, f"Regex scan took {duration:.4f}s")

if __name__ == '__main__':
    unittest.main()
