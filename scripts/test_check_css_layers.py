#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests of check_css_layers.py.

Usage: test_check_css_layers.py
"""

import unittest

import check_css_layers as check

SOURCE = "/* The order. */\n@layer reset, tokens, components;\n"
INDEX = '<html><head><link rel="stylesheet" crossorigin href="/assets/index-abc.css"></head></html>'
GOOD = "@layer reset,tokens,components;@font-face{font-family:x}@layer reset{*{margin:0}}"


class CheckCssLayers(unittest.TestCase):
    def test_accepts_the_layer_statement_first(self):
        self.assertEqual(check.problems(SOURCE, INDEX, {"assets/index-abc.css": GOOD}), [])

    def test_rejects_a_rule_before_the_statement(self):
        built = {"assets/index-abc.css": "@font-face{font-family:x}" + GOOD}
        self.assertIn("no layer statement", check.problems(SOURCE, INDEX, built)[0])

    def test_rejects_another_order(self):
        built = {"assets/index-abc.css": "@layer tokens,reset,components;" + GOOD}
        self.assertIn("expected ['reset', 'tokens', 'components']", check.problems(SOURCE, INDEX, built)[0])

    def test_rejects_an_inline_style(self):
        index = INDEX.replace("<head>", "<head><style>@layer reset;</style>")
        found = check.problems(SOURCE, index, {"assets/index-abc.css": GOOD})
        self.assertEqual(len(found), 1)
        self.assertIn("inline <style>", found[0])

    def test_rejects_a_page_without_style_sheet(self):
        self.assertIn("no style sheet", check.problems(SOURCE, "<html></html>", {})[0])

    def test_rejects_a_source_without_statement(self):
        self.assertIn("layers.css", check.problems(".a{}", INDEX, {})[0])


if __name__ == "__main__":
    unittest.main()
