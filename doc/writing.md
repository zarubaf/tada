# Writing guide

This guide applies to all prose in this repository: documentation, ADRs, backlog items and pull request descriptions.
It applies to humans and to LLM agents.
Vale and mdformat check most of these rules (see [ADR 0012](adr/0012-documentation.md)).

## Language

- Use US English spelling: "organization", "authorize", "license".
- Use the terms in the [glossary](glossary.md) with exactly one meaning each.
  If you need a new term, add it to the glossary in the same commit.
- Do not translate German domain names such as "Verkehrsplan" or "Organisationskomitee".
  The glossary defines them.

## Simplified Technical English

The project uses a subset of ASD-STE100 (Simplified Technical English).

- Use one word for one meaning, and one verb for one action.
- Use the plain verbs: check, make sure, start, stop, use, show, find, change, remove, need.
- Use the active voice and name the actor: "The worker sends the reminder."
- Use simple tenses. Do not use "has been" or "have done".
- Write at most 20 words in an instruction and at most 25 words in a description.
- Write one instruction per sentence.
- Start a warning with the instruction: "Do not run this on main. It rewrites history."
- Keep facts, numbers, conditions and scope. Precision wins over style.

## Markdown layout

- Write one sentence per line. Git diffs then show the changed sentence only.
- Use a numbered list for a sequence of three or more steps.
- Use a bulleted list for three or more parallel items.
- Start each file with one `#` heading.
- Use relative links to other files in the repository.

## Checks

Run all documentation checks:

```sh
mise run check:docs
```

Format all Markdown files in place:

```sh
mise run fmt
```

Show all Vale findings, including warnings:

```sh
mise run lint-prose
```

Only Vale errors fail the check.
Warnings (passive voice, sentence length, preferred verbs) are for the author to judge.
