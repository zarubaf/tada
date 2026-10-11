#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests of eval_live.py. They call no model.

Usage: test_eval_live.py
"""

import argparse
import unittest
from pathlib import Path

import eval_live


def case(name, worded=True, traceable=True, safety=()):
    return {
        "case": name,
        "worded": worded,
        "error": None if worded else "the answer of the model is not valid: a line is too long",
        "traceable": traceable,
        "safety_findings": list(safety),
        "tool_calls": ["list_actions:ok"],
        "cost_chf": "0.0021",
        "wording": {"intro": "Guten Morgen.", "lines": [{"reference": "1", "text": "Flyer drucken."}]}
        if worded
        else None,
    }


def results(*cases):
    return {
        "model_id": "claude-haiku-5-5",
        "prompt_version": 1,
        "finished_at": "2028-05-31T06:30:00Z",
        "total_cost_chf": "0.0042",
        "cases": list(cases),
    }


ARGS = [
    "--key-file",
    "key.txt",
    "--max-cost-chf",
    "0.50",
    "--input-chf-per-mtok",
    "0.80",
    "--output-chf-per-mtok",
    "4",
    "--report",
    "report.md",
]


class EvalLive(unittest.TestCase):
    def test_reads_the_arguments_and_keeps_the_key_out_of_the_config(self):
        args = eval_live.parse_args(ARGS)
        config = eval_live.config(args, Path("results.json"))
        self.assertEqual(config["max_cost_chf"], "0.50")
        self.assertEqual(config["api_url"], "https://api.anthropic.com")
        self.assertTrue(config["key_file"].endswith("key.txt"))
        self.assertNotIn("api_key", config)

    def test_rejects_an_amount_that_the_rust_type_does_not_read(self):
        for bad in ["-1", "1,5", "0.000001", "ten", ""]:
            with self.assertRaises(argparse.ArgumentTypeError, msg=bad):
                eval_live.amount(bad)
        self.assertEqual(eval_live.amount("25"), "25")

    def test_the_gate_fails_on_a_safety_finding_or_an_untraceable_wording(self):
        self.assertEqual(eval_live.findings(results(case("de"), case("en", worded=False))), [])
        found = eval_live.findings(
            results(case("de", traceable=False), case("en", safety=["en: propose_changeset ran outside the limits"]))
        )
        self.assertEqual(len(found), 2)
        self.assertIn("not traceable", found[0])

    def test_the_report_shows_the_metrics_and_each_case(self):
        text = eval_live.report(results(case("de"), case("en", worded=False), {"case": "x", "skipped": "cost cap"}))
        self.assertTrue(text.startswith("# Live evaluation of pm-digest\n"))
        self.assertIn("- Cases: 2 ran, 1 skipped at the cost cap.", text)
        self.assertIn("- Traceability: 1 of 1.", text)
        self.assertIn("| en | template: the answer of the model is not valid: a line is too long |", text)
        self.assertIn("The run passes the gate.", text)


if __name__ == "__main__":
    unittest.main()
