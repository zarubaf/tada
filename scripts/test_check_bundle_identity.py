#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests of check_bundle_identity.py.

Usage: test_check_bundle_identity.py
"""

import unittest

import check_bundle_identity as check

CLEAN = 'const a="2d21ae24ef18da7c";'


class CheckBundleIdentity(unittest.TestCase):
    def test_accepts_the_opaque_build_id(self):
        self.assertEqual(check.problems("0.3.0", "1a2b3c4", {"assets/index.js": CLEAN}), [])

    def test_rejects_the_version(self):
        found = check.problems("0.3.0", "1a2b3c4", {"assets/index.js": 'const v="0.3.0";'})
        self.assertEqual(found, ["assets/index.js shows the version"])

    def test_rejects_the_version_with_a_v(self):
        found = check.problems("0.3.0", "1a2b3c4", {"assets/index.js": "`tada v0.3.0`"})
        self.assertEqual(found, ["assets/index.js shows the version"])

    def test_rejects_the_commit(self):
        found = check.problems("0.3.0", "1a2b3c4", {"assets/index.js": "`tada · 1a2b3c4`"})
        self.assertEqual(found, ["assets/index.js shows the commit"])

    def test_ignores_a_longer_number_or_hex_string(self):
        text = 'const a="10.3.0.1",b="ff1a2b3c4e";'
        self.assertEqual(check.problems("0.3.0", "1a2b3c4", {"assets/index.js": text}), [])

    def test_checks_no_commit_without_one(self):
        self.assertEqual(check.problems("0.3.0", None, {"assets/index.js": "1a2b3c4"}), [])


if __name__ == "__main__":
    unittest.main()
