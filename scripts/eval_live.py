#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Run the evaluation set of the digest wording against the configured model (ADR 0048, ADR 0077).

The script starts the ignored Rust test `live_eval` of `crates/tada/tests/pm_digest_eval.rs`.
So the live run uses the same runner, prompt and checks as the product.
The test stops before the cost of the run can reach the cap.
The script writes a Markdown report and fails if a safety finding exists or a wording is not traceable.
It calls the model provider and costs money, so it never runs in CI.

Usage: eval_live.py --key-file PATH --max-cost-chf AMOUNT --input-chf-per-mtok AMOUNT
                    --output-chf-per-mtok AMOUNT --report PATH [--api-url URL]
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
AMOUNT = re.compile(r"\d+(\.\d{1,5})?")
DEFAULT_API_URL = "https://api.anthropic.com"
TEST = ["cargo", "test", "--locked", "-p", "tada", "--test", "pm_digest_eval"]
TEST += ["--", "--ignored", "--exact", "live_eval"]


def amount(text: str) -> str:
    """An amount in CHF as the Rust type `Chf` reads it: digits with at most 5 decimal places."""
    if not AMOUNT.fullmatch(text):
        raise argparse.ArgumentTypeError(f"{text!r} is not an amount such as 0.50")
    return text


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--key-file", required=True, type=Path, help="the file with the API key")
    parser.add_argument("--max-cost-chf", required=True, type=amount, help="the cost cap of the whole run")
    for kind in ("input", "output"):
        parser.add_argument(
            f"--{kind}-chf-per-mtok", required=True, type=amount, help=f"the price of a million {kind} tokens"
        )
    parser.add_argument("--report", required=True, type=Path, help="the Markdown report to write")
    parser.add_argument("--api-url", default=DEFAULT_API_URL, help="the base URL of the Messages API")
    return parser.parse_args(argv)


def config(args: argparse.Namespace, results: Path) -> dict:
    """The config file of the Rust test. It holds the path of the key, never the key."""
    return {
        "api_url": args.api_url,
        "key_file": str(args.key_file.resolve()),
        "max_cost_chf": args.max_cost_chf,
        "input_chf_per_mtok": args.input_chf_per_mtok,
        "output_chf_per_mtok": args.output_chf_per_mtok,
        "report": str(results),
    }


def findings(results: dict) -> list[str]:
    """The reasons why the run fails the gate of ADR 0048. An empty list means that it passes."""
    found = []
    for case in results["cases"]:
        name = case["case"]
        found.extend(case.get("safety_findings", []))
        if case.get("worded") and not case.get("traceable"):
            found.append(f"{name}: the wording is not traceable to the items")
    return found


def report(results: dict) -> str:
    """The Markdown report of a run."""
    cases = results["cases"]
    ran = [case for case in cases if "skipped" not in case]
    worded = [case for case in ran if case.get("worded")]
    traceable = [case for case in worded if case.get("traceable")]
    safety = sum(len(case.get("safety_findings", [])) for case in ran)
    share = f"{len(traceable)} of {len(worded)}" if worded else "no wording"
    lines = [
        "# Live evaluation of pm-digest",
        "",
        f"- Model: `{results['model_id']}`, prompt version {results['prompt_version']}.",
        f"- Finished at {results['finished_at']}, total cost CHF {results['total_cost_chf']}.",
        f"- Cases: {len(ran)} ran, {len(cases) - len(ran)} skipped at the cost cap.",
        f"- Worded: {len(worded)} of {len(ran)}. The other cases use the template.",
        f"- Traceability: {share}.",
        f"- Safety findings: {safety}.",
        "",
        "| Case | Result | Tool calls | Cost (CHF) |",
        "| --- | --- | --- | --- |",
    ]
    for case in cases:
        if "skipped" in case:
            lines.append(f"| {case['case']} | skipped | | |")
            continue
        result = "worded" if case.get("worded") else f"template: {case.get('error')}"
        tools = ", ".join(case.get("tool_calls", [])) or "none"
        lines.append(f"| {case['case']} | {result} | {tools} | {case['cost_chf']} |")
    for case in worded:
        lines += ["", f"## {case['case']}", "", case["wording"]["intro"], ""]
        lines += [f"- {line['reference']}: {line['text']}" for line in case["wording"]["lines"]]
    found = findings(results)
    lines += ["", "## Gate", ""]
    lines += [f"- {line}" for line in found] if found else ["The run passes the gate."]
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    with tempfile.TemporaryDirectory() as folder:
        results_path = Path(folder) / "results.json"
        config_path = Path(folder) / "config.json"
        config_path.write_text(json.dumps(config(args, results_path)))
        env = {**os.environ, "TADA_EVAL_LIVE_CONFIG": str(config_path)}
        # The command is fixed; only the environment holds a path.
        run = subprocess.run(TEST, cwd=ROOT, env=env, check=False)  # noqa: S603
        if run.returncode != 0 or not results_path.exists():
            print("the live evaluation did not finish", file=sys.stderr)
            return 1
        results = json.loads(results_path.read_text())
    args.report.write_text(report(results))
    found = findings(results)
    for line in found:
        print(line, file=sys.stderr)
    print(f"report: {args.report}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
