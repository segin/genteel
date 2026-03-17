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
        self.assertIsInstance(audit_tool.SECRET_PATTERN_COMBINED, re.Pattern, f"Combined pattern is not compiled")
        self.assertIsInstance(audit_tool.TODO_PATTERN, re.Pattern)
        self.assertIsInstance(audit_tool.UNSAFE_PATTERN, re.Pattern)

    def test_aws_key_match(self):
        pattern = audit_tool.SECRET_PATTERN_COMBINED
        match = pattern.search("AKIA" + "IOSFODNN7EXAMPLE")
        self.assertIsNotNone(match)
        self.assertEqual(match.lastgroup, "AWS_Key")
        self.assertIsNone(pattern.search("AKIA" + "IOSFODNN7EXAMPL")) # Too short

    def test_private_key_match(self):
        pattern = audit_tool.SECRET_PATTERN_COMBINED
        match = pattern.search("-----BEGIN RSA PRIVATE " + "KEY-----")
        self.assertIsNotNone(match)
        self.assertEqual(match.lastgroup, "Private_Key")

    def test_generic_token_match(self):
        pattern = audit_tool.SECRET_PATTERN_COMBINED
        match = pattern.search('token=' + '"abcdefghijklmnopqrstuvwxyz0123456789"')
        self.assertIsNotNone(match)
        self.assertEqual(match.lastgroup, "Generic_Token")

    def test_api_key_match(self):
        pattern = audit_tool.SECRET_PATTERN_COMBINED
        match = pattern.search("api_" + "key=" + "'12345'")
        self.assertIsNotNone(match)
        self.assertEqual(match.lastgroup, "API_Key")
        match2 = pattern.search('API-' + 'KEY: ' + '"secret"')
        self.assertIsNotNone(match2)
        self.assertEqual(match2.lastgroup, "API_Key")

    def test_password_match(self):
        pattern = audit_tool.SECRET_PATTERN_COMBINED
        match = pattern.search("pass" + "word = " + "'12345'")
        self.assertIsNotNone(match)
        self.assertEqual(match.lastgroup, "Password")

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
