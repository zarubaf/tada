# 0047. Model adapter details

- Status: Proposed
- Date: 2026-10-06

## Context

Slice 2 brings AI intake inside tada (ADR 0040) and the AI PM.
ADR 0010 defines the `Model` port and the limits of AI authority.
PRODUCT.md sets a target of CHF 25 per month for metered AI, and a cap must not block manual planning.
Model names and prices change often, so they must not be fixed in code.

## Decision

Tasks and models:

- Each AI task has a name, for example `extract-proposals`, `answer-question`, `draft-document` and `pm-digest`.
- A setting maps each task to a model ID. Extraction and digests use a small, inexpensive model; drafting and complex answers use a larger model.
- The operator changes a model by a setting, without a release.

Prompts:

- Prompts are files in the repository, one folder for each task, with a version number.
- Each call records the task, the prompt version and the model ID (ADR 0010).
- A prompt change needs a pull request and a run of the evaluation set (ADR 0048).

Tools:

- The intake inside tada uses the same tool set as the MCP server (ADR 0040), through `AiCaller` (ADR 0039).
- So extraction behaves the same, whether a member's own agent or tada's model adapter does it.

Cost control:

- Each organization and each event has a monthly cap in Swiss francs. The adapter estimates the cost of each call from the token counts and a price table in the settings.
- At the cap, the adapter pauses optional AI work and shows this in the UI. Manual planning continues.
- The UI shows the usage of each event.

Caching and reliability:

- The adapter caches results by task, prompt version, model ID and source version. The same source is not extracted twice.
- Each call has a timeout. The adapter retries a temporary error at most twice with a backoff.
- A failed extraction leaves the source in the review queue with a note; it never blocks other work.

Data protection:

- The adapter works only after the checks of ADR 0045 for the configured provider.

## Consequences

- A model change is a setting and an evaluation run, not a code change.
- AI cost stays inside a known limit for each organization.
- The in-app intake and the MCP intake share their tools and their tests.

## Alternatives

- One large model for all tasks: higher cost for simple extraction.
- Model IDs in code: each model change needs a release.
- No cache: the same email would be extracted again after each retry.
