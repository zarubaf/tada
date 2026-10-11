#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests of check_versions.py.

Usage: test_check_versions.py
"""

import unittest

import check_versions as check


def cargo(version):
    return {"workspace": {"package": {"version": version}}}


class CheckVersions(unittest.TestCase):
    def test_accepts_the_same_version(self):
        self.assertEqual(check.problems(cargo("0.1.0"), {"version": "0.1.0"}), [])

    def test_rejects_a_mismatch(self):
        found = check.problems(cargo("0.2.0"), {"version": "0.1.0"})
        self.assertEqual(len(found), 1)
        self.assertIn("differs", found[0])

    def test_rejects_a_missing_web_version(self):
        self.assertIn("no version", check.problems(cargo("0.1.0"), {})[0])

    def test_rejects_a_version_that_is_not_semantic(self):
        self.assertIn("not MAJOR.MINOR.PATCH", check.problems(cargo("1.0"), {"version": "1.0"})[0])


if __name__ == "__main__":
    unittest.main()
