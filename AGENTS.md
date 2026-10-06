# Timeless constraints (not a checklist)

When two principles collide, pick the one that cuts future cost in THIS codebase.
HARD RULE: refactor to the principle FIRST, then change behavior.

1. Separation of Concerns - one kind of work per part (UI / domain / persistence / infra).
Root principle.
1. Encapsulation / Information Hiding - small stable contract; hide internals.
2. High Cohesion + Loose Coupling - change-together lives together; independents talk narrow.
3. DRY - one authoritative representation of each piece of knowledge (not every similar line). Avoid over-DRY.
4. KISS - simplest design that works; complexity is the long-term tax.
5. Single Responsibility - one reason to change.
6. Depend on Abstractions - policy doesn't depend on details; both depend on contracts.
7. YAGNI - no speculative features, frameworks, or later" hooks.
8. Composition over Inheritance - assemble pieces; don't grow fragile hierarchies.9.     Open/Closed (with discipline) - extend at stable boundaries; only where change showed up twice.
10. Honorable: Law of Demeter • fail fast / illegal states unrepresentable • optimize for deletion
11. Unix do-one-thing + compose.
12. Treat as constraints. Violate slogans when judgment says so.