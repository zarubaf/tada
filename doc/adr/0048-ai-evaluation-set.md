# 0048. AI evaluation set

- Status: Accepted
- Date: 2026-10-06

## Context

AI extraction and drafting can fail silently: a wrong date, a lost condition, a made-up approval.
ARCHITECTURE.md lists the cases that the evaluation set must contain.
The roadmap requires that every asserted fact traces to an accepted field or an exact source version.
The repository is public, so fixtures must not contain real data.
Live model calls cost money and give different answers each time.

## Decision

Fixtures:

- `eval/` contains cases with invented data. Each case has an input (a message, a document or a conversation), the event state before, and the expected proposals.
- The cases cover at least: contradictory dates, conditional promises, German and English messages, duplicate emails, wrong-event routing, prompt injection in a message or a document, private sources, and missing evidence.
- A real problem from operation becomes a new case, with invented names.

Metrics:

- Traceability: each proposed value links to a passage of its source. The target is 100%.
- Precision: the share of proposals that match an expected proposal.
- Recall: the share of expected proposals that the model finds.
- Safety: no case may produce an accepted change, a send or a tool call outside the `AiCaller` limits (ADR 0039). The target is zero findings.

Runs:

- CI runs the evaluation on each pull request with recorded model responses. This checks the parsing, the tool handling and the scoring without cost.
- A live run calls the configured models. It runs on each prompt or model change (ADR 0047) and once a week, with a fixed cost cap.
- A Python script (ADR 0034) runs the cases and writes a report.

Gate:

- A prompt or model change merges only if traceability stays at 100%, safety findings stay at zero, and precision and recall do not drop by more than the documented tolerance.

## Consequences

- A prompt change shows its effect before it reaches members.
- The recorded responses make CI fast and free; the live run catches model changes.
- The set grows with each real problem.

## Alternatives

- Manual checks only: regressions stay unnoticed.
- Live calls in each CI run: cost and unstable results.
