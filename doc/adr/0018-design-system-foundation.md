# 0018. Design system foundation and tokens

- Status: Accepted
- Date: 2026-10-06

## Context

The product owner wants a clean product UI that does not look generated, with Linear and GitHub as quality references.
tada is a product UI: lists, tables, forms and review queues, not a landing page.
Agents write most of the UI code. Without one fixed vocabulary, each screen drifts.
We studied two agent skills for design guidance: taste-skill and impeccable (see [doc/design/principles.md](../design/principles.md)).
Their product-UI guidance is useful, but taste-skill targets landing pages, and impeccable runs a downloaded binary.

## Decision

- tada has one design system. [doc/design/](../design/principles.md) describes it.
- Design tokens are the single source for color, type, spacing, radius, elevation, motion and z-index.
- Until code exists, [doc/design/tokens.md](../design/tokens.md) is the authority for token names and values.
  When the web client exists, `apps/web/src/styles/tokens.css` becomes the authority, and `tokens.md` keeps only the rules and the reasons.
- Tokens have two levels:
  1. Semantic tokens, for example `--color-text-muted` or `--space-4`. Components use only these.
  2. No raw palette level. A raw value appears only once, in the token definition.
- tada has a light and a dark theme. The default follows `prefers-color-scheme`. A member can override it in the settings.
- tada has two densities: `comfortable` (default on touch devices) and `compact` (default on desktop with a fine pointer).
  Density changes spacing and row height, never the font size of body text below 14 px.
- The design system has one accent color (petrol, hue 220). The accent marks primary actions, the current selection, links and focus only.
- The state of knowledge (accepted, proposed, assumption, unknown, conflict) has its own visual vocabulary. It never uses color alone.
- We install no third-party design skill now. [doc/design/principles.md](../design/principles.md) carries the applicable rules, with attribution.

## Consequences

- Agents and humans use the same token names, so a review can find a raw value fast.
- The state-of-knowledge vocabulary makes tada recognizable without decoration.
- A brand change is a change of token values only.

## Alternatives

- An existing design system such as Primer or Radix Themes: fast, but tada would look like GitHub or like many Radix apps, and we would override most tokens.
- A token build tool such as Style Dictionary with DTCG JSON: useful with many consumers. Today only the web client consumes tokens (YAGNI).
- Installing taste-skill: its main skill excludes product UI, and some rules conflict with tada, for example random fake data.
- Installing impeccable: it downloads and runs a binary that we cannot review.
