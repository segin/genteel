import unittest
import sys
import os
import re

# Add parent directory and scripts directory to path to allow importing audit_tool
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.append(project_root)
sys.path.append(os.path.join(project_root, 'scripts'))

try:
    import audit_tool
except ImportError:
    print(f"DEBUG: sys.path = {sys.path}")
    raise

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
            self.assertTrue(pattern.search("AKIA" + "IOSFODNN7EXAMPLE"))
            self.assertFalse(pattern.search("AKIA" + "IOSFODNN7EXAMPL")) # Too short

    def test_private_key_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("Private Key")
        if pattern:
            self.assertTrue(pattern.search("-----BEGIN RSA PRIVATE " + "KEY-----"))

    def test_generic_token_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("Generic Token")
        if pattern:
            self.assertTrue(pattern.search('token=' + '"abcdefghijklmnopqrstuvwxyz0123456789"'))

    def test_api_key_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("API Key")
        if pattern:
            self.assertTrue(pattern.search("api_" + "key=" + "'12345'"))
            self.assertTrue(pattern.search('API-' + 'KEY: ' + '"secret"'))

    def test_password_match(self):
        pattern = audit_tool.SECRET_PATTERNS.get("Password")
        if pattern:
            self.assertTrue(pattern.search("pass" + "word = " + "'12345'"))

    def test_unsafe_pattern_match(self):
        pattern = audit_tool.UNSAFE_PATTERN
        self.assertTrue(pattern.search("un" + "safe {"))
        self.assertTrue(pattern.search("un" + "safe  {"))

    def test_todo_pattern_match(self):
        pattern = audit_tool.TODO_PATTERN
        self.assertTrue(pattern.search("TO" + "DO: fix me"))
        self.assertTrue(pattern.search("FIX" + "ME: broken"))
        self.assertTrue(pattern.search("X" + "XX: critical"))

if __name__ == '__main__':
    unittest.main()
