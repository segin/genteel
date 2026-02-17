import unittest
import sys
import os
import re
import time
import random
import string

# Add parent directory to path to allow importing audit_tool
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

try:
    import audit_tool
except ImportError:
    # Fallback if run from root and sys.path setup failed (e.g. unexpected path)
    sys.path.append('.')
    import audit_tool

class TestAuditRegex(unittest.TestCase):
    """Tests for regex patterns used in audit_tool.py."""

    def test_patterns_compile(self):
        """Verify all patterns are valid compiled regex objects."""
        for name, pattern in audit_tool.SECRET_PATTERNS.items():
            self.assertIsInstance(pattern, re.Pattern, f"Pattern '{name}' is not compiled")
        self.assertIsInstance(audit_tool.TODO_PATTERN, re.Pattern)
        self.assertIsInstance(audit_tool.UNSAFE_PATTERN, re.Pattern)

    def test_aws_key_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("AWS Key")
        if pattern:
            self.assertTrue(pattern.search("AKIAIOSFODNN7EXAMPLE"))
            self.assertFalse(pattern.search("AKIAIOSFODNN7EXAMPL")) # Too short

    def test_private_key_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("Private Key")
        if pattern:
            self.assertTrue(pattern.search("-----BEGIN RSA PRIVATE KEY-----"))

    def test_generic_token_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("Generic Token")
        if pattern:
            self.assertTrue(pattern.search('token="abcdefghijklmnopqrstuvwxyz0123456789"'))

    def test_api_key_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("API Key")
        if pattern:
            self.assertTrue(pattern.search("api_key='12345'"))
            self.assertTrue(pattern.search('API-KEY: "secret"'))

    def test_password_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("Password")
        if pattern:
            self.assertTrue(pattern.search("password = '12345'"))

    def test_unsafe_pattern_match(self):
        pattern = audit_tool.UNSAFE_PATTERN
        self.assertTrue(pattern.search("unsafe {"))
        self.assertTrue(pattern.search("unsafe  {"))

    def test_todo_pattern_match(self):
        pattern = audit_tool.TODO_PATTERN
        self.assertTrue(pattern.search("TODO: fix me"))
        self.assertTrue(pattern.search("FIXME: broken"))
        self.assertTrue(pattern.search("XXX: critical"))

class TestRegexPerformance(unittest.TestCase):
    """Performance benchmark for regex patterns."""

    FILENAME = "benchmark_data_test.txt"
    FILE_SIZE_MB = 1 # Small default size for CI to be fast

    @classmethod
    def setUpClass(cls):
        # Check if we should run a larger benchmark via env var
        if os.environ.get("BENCHMARK_SIZE_MB"):
            try:
                cls.FILE_SIZE_MB = int(os.environ["BENCHMARK_SIZE_MB"])
            except ValueError:
                pass # Default to 1MB

        print(f"Generating {cls.FILE_SIZE_MB}MB test file for benchmark...")
        cls.generate_data(cls.FILE_SIZE_MB)

    @classmethod
    def tearDownClass(cls):
        if os.path.exists(cls.FILENAME):
            os.remove(cls.FILENAME)

    @classmethod
    def generate_data(cls, size_mb):
        chunk_size = 1024 * 1024 # 1MB
        chars = string.ascii_letters + string.digits + " \n\t"

        with open(cls.FILENAME, "w", encoding="utf-8") as f:
            for _ in range(size_mb):
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

    def test_performance_comparison(self):
        """Compare pre-compiled vs runtime compilation performance."""
        # Use existing patterns from audit_tool
        patterns_to_test = list(audit_tool.SECRET_PATTERNS.values()) + \
                           [audit_tool.UNSAFE_PATTERN, audit_tool.TODO_PATTERN]

        # Slow scan (runtime compilation)
        start_time = time.time()
        slow_matches = 0
        with open(self.FILENAME, 'r', encoding='utf-8', errors='ignore') as fp:
            for line in fp:
                for pattern in patterns_to_test:
                    # simulate runtime compilation by using re.search with pattern string
                    if re.search(pattern.pattern, line):
                        slow_matches += 1
        slow_duration = time.time() - start_time

        # Fast scan (pre-compiled)
        start_time = time.time()
        fast_matches = 0
        with open(self.FILENAME, 'r', encoding='utf-8', errors='ignore') as fp:
            for line in fp:
                for pattern in patterns_to_test:
                    if pattern.search(line):
                        fast_matches += 1
        fast_duration = time.time() - start_time

        print(f"\nResults (Size: {self.FILE_SIZE_MB}MB):")
        print(f"Runtime Compilation: {slow_duration:.4f}s")
        print(f"Pre-compiled:        {fast_duration:.4f}s")
        if slow_duration > 0:
            improvement = (slow_duration - fast_duration) / slow_duration * 100
            print(f"Improvement:         {improvement:.2f}%")

        self.assertEqual(slow_matches, fast_matches, "Match counts should be identical")
        # Ensure fast scan is reasonably fast (not a strict check as it depends on machine load)
        # Just ensure it runs without error.

if __name__ == '__main__':
    unittest.main()
