# Design system

These documents describe how the tada web client looks and behaves.
The ADRs 0018 to 0024 record the decisions and their reasons; these documents give the rules and the values.

| Document                                                     | Content                                                            |
| ------------------------------------------------------------ | ------------------------------------------------------------------ |
| [principles.md](principles.md)                               | Principles, the anti-slop checklist and the references             |
| [tokens.md](tokens.md)                                       | Color, type, spacing, radius, elevation, motion and z-index values |
| [layout-and-responsiveness.md](layout-and-responsiveness.md) | App shell, breakpoints, page templates and navigation              |
| [accessibility.md](accessibility.md)                         | Accessibility rules, locale formats and the test procedure         |
| [components.md](components.md)                               | Core components and the patterns for the main screens              |

Before you build or change a screen:

1. Read the principles and the anti-slop checklist.
2. Use only the tokens and the components in these documents.
3. If you need a new token or component, add it here in the same pull request.
4. Run the checks of ADR 0024 and the manual checks in [accessibility.md](accessibility.md).
