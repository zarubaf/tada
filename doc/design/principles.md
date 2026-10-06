# Design principles

tada is a work tool for volunteers.
The interface must disappear into the task: a member opens it, finds the open item, acts and leaves.

## Principles

1. **Evidence before decoration.**
   Each value shows its state of knowledge: accepted, proposed, assumption, unknown or conflict.
   This vocabulary is the visual identity of tada. Decoration is not.
2. **Calm by default, loud only for exceptions.**
   Most of the screen is neutral. Color marks actions, selection, focus and real problems.
   If everything stands out, nothing does.
3. **Familiar affordances.**
   A button looks like a button, a link like a link, a table like a table.
   tada does not invent controls for standard tasks.
4. **Keyboard and phone are both first-class.**
   The desktop user works with the keyboard; the volunteer works with a thumb.
   Each screen works for both (see [layout-and-responsiveness.md](layout-and-responsiveness.md)).
5. **Density with structure.**
   The PM sees many items at once. Alignment, tabular numbers and consistent row heights make density readable.
6. **The words are the interface.**
   Labels name actions with a verb and an object: „Vorschlag annehmen“, not "OK".
   Errors name the problem and the fix.
7. **One vocabulary.**
   The same thing looks the same on each screen. If a "Save" button looks different in two places, one is wrong.

## Anti-slop checklist

Check each screen against this list before review.
Each item is a yes-or-no question.

Surfaces and color:

- [ ] No gradient backgrounds and no gradient text.
- [ ] No glow, no glass or blur effect, no noise texture.
- [ ] One accent color only (`--color-accent`), used only for primary actions, selection, links and focus.
- [ ] Status colors appear only with a real status, and always with an icon or a label.
- [ ] Shadows only on overlays (menus, popovers, dialogs, sheets). Panels use borders.
- [ ] No colored side border thicker than 1 px on cards, list items or alerts.
- [ ] Radius values follow [tokens.md](tokens.md). No pill-shaped buttons.

Layout and structure:

- [ ] No grid of identical cards with icon, heading and text as the page structure.
- [ ] No cards inside cards.
- [ ] No small uppercase label ("eyebrow") above a heading.
- [ ] No big-number tiles unless the number leads to an action and links to the items behind it.
- [ ] No section numbers (01, 02, 03) unless the order matters to the reader.
- [ ] A modal dialog only for a destructive confirmation or a task that must block the rest of the screen. Otherwise use inline editing or a sheet.

Content:

- [ ] Sentence case for all headings, labels and buttons.
- [ ] No exclamation marks, no "Oops", no jokes in system messages.
- [ ] No emoji and no decorative icons. Each icon has a meaning that the label repeats.
- [ ] No sparkle or magic icon for AI content. AI output is a proposal and uses the proposal style.
- [ ] No lorem ipsum. Fixtures use invented but realistic content: long German names, umlauts, long titles and empty fields.
- [ ] No invented numbers that look like real data in demos or screenshots.
- [ ] No placeholder text as a label.

States and motion:

- [ ] Each interactive component has these states: default, hover, focus, pressed, disabled. Data views also have loading, empty and error.
- [ ] Loading uses skeletons in the shape of the content, not a spinner in the middle of the content.
- [ ] Motion only shows a change of state, in 120 ms to 240 ms. No entrance animations on page load. No bounce.
- [ ] Browser surfaces follow the theme: `color-scheme`, `accent-color`, `::selection` and `caret-color`. No custom scrollbars.

## References

These products show the quality we want. We take principles from them, not their look.

| Reference                                                    | What we learn from it                                                          |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| [Linear](https://linear.app)                                 | Keyboard-first work, a command menu, restraint, dense lists that stay readable |
| [GitHub Primer](https://primer.style)                        | Accessible tokens, data tables, review flows, clear status labels              |
| [GOV.UK Design System](https://design-system.service.gov.uk) | Forms, error summaries, plain-language messages, tested accessibility          |
| [Stripe Dashboard](https://dashboard.stripe.com)             | Tables with numbers, detail panels next to lists, calm color                   |
| [Things](https://culturedcode.com/things/)                   | A calm mobile task interface with few, clear actions                           |
| [Vercel Geist](https://vercel.com/geist)                     | Disciplined tokens for type and color                                          |

## Sources of the agent guidance

We studied two agent skills for frontend design on 2026-10-06 and did not install them (see ADR 0018):

- [taste-skill](https://github.com/leonxlnx/taste-skill), MIT, commit `ce26fc25c0e5e8cab638f883de62d9a86ee5e45b`.
- [impeccable](https://github.com/pbakaus/impeccable), Apache-2.0, commit `cf3d2fa07d3ad1814ac5fbbbb5b2043b795eaef1`.

We adopted, in our own words:

- The bans on generic patterns (gradients, glow, eyebrows, identical card grids, nested cards, decorative motion).
- From the impeccable "Operate" mode: one font family, a fixed rem scale with a ratio of about 1.125 to 1.2, 150 ms to 250 ms motion, complete component states and a restrained color strategy.
- The checks for contrast of text, placeholders and controls.

We deviate on purpose:

| Guidance                                              | Our decision                      | Reason                                                            |
| ----------------------------------------------------- | --------------------------------- | ----------------------------------------------------------------- |
| Real imagery on each page (taste-skill)               | No stock images                   | tada is a work tool; images do not help a task.                   |
| Tailwind as the default (taste-skill)                 | CSS Modules and tokens (ADR 0019) | Raw values in utility classes bypass the tokens.                  |
| An animation library (taste-skill)                    | CSS transitions only              | Motion only shows state changes.                                  |
| Phosphor icons first (taste-skill)                    | Tabler icons (ADR 0021)           | Phosphor had no release since May 2025.                           |
| Realistic, random fake data (taste-skill)             | Invented data, clearly fixtures   | tada is evidence-first; fake data that looks real is a risk.      |
| Bold, maximal design (impeccable default)             | Restraint                         | the impeccable own "Operate" mode recommends this for product UI. |
| `PRODUCT.md` and `DESIGN.md` in the root (impeccable) | `doc/design/`                     | The repository already has a documentation structure.             |
