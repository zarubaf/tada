# 0081. Milestones, decisions, risks and requirements

- Status: Proposed
- Date: 2026-10-13
- Amends: [0067](0067-workstreams-and-review-routing.md), [0068](0068-actions-and-commitments.md)

## Context

A club committee follows more than actions and commitments.
It sets milestones, records decisions, watches risks and tracks requirements, for example from an authority.
ADR 0049 names these kinds, and Slice 2d delivers them.
Actions also need a start date and a milestone, so that one page can show the plan of an event.
MCP clients must propose all of these records, and a new workstream ([ADR 0080](0080-workstream-proposals.md), [#75](https://github.com/zarubaf/tada/issues/75)).
The MCP tools must also serialize an empty `statuses` list ([#73](https://github.com/zarubaf/tada/issues/73)).

## Decision

### Storage

- Each kind has its own typed table in event scope, with a record version, audit entries, a readable ID ([ADR 0038](0038-ids-and-time.md)), an owner and an optional workstream.
  The prefixes are `MIL`, `DEC`, `RSK` and `REQ`.
- The owner is a member of the event with the contributor or manager role.
  Any other owner returns `validation-failed` with the field code `unknown-member`.
- A closed workstream, or a workstream of another event, returns the field code `unknown-record`, as in ADR 0067.
- A change to the current status returns `invalid-transition`.
- `record_evidence` gets one column for each new kind.
  The `CHECK` that exactly one is set covers them.
  An accepted proposal writes evidence as for actions.

### Milestone

- Fields: title (at most 200 characters), optional description (at most 4000), date, owner, optional workstream and status.
- Statuses: `open`, `reached` and `canceled`.
  Someone sets `reached` by hand.
- Transitions: `open` to `reached`, `open` to `canceled`, and `reached` to `open`.
  `canceled` is final.
- tada calculates two warnings and does not store them:
  - An action of the milestone is open, in progress or blocked, and its due date is after the date of the milestone.
  - The date of the milestone is before today in the time zone of the event, and its status is `open`.

### Action (changed)

- An action gets an optional start date (`start_date`) and at most one optional milestone.
- If both dates are set, the start date is not after the due date.
  Otherwise the field code is `start-after-due`.
- The milestone is one of the same event with a status other than `canceled` (`unknown-record`).
  An existing link stays when the milestone is canceled later.
- A direct command changes the start date and the milestone of an existing action, with the rights to change the action.
  A proposal sets the start date only in `CreateAction`.
  The milestone of an existing action changes through `ChangeActionMilestone`.

### Decision

- Fields: wording (at most 2000 characters), date of the decision, decided by, owner, optional workstream and status.
- Decided by is at least one person or institution of the organization.
  A link table `decision_party` keeps them.
- The wording, the date and the parties never change after the creation.
  A changed decision is a new decision that supersedes the old one.
- Statuses: `in-force`, `superseded` and `revoked`.
- Transitions, both final:
  - `in-force` to `superseded` names `superseded_by`.
    That is another decision of the same event that is in force.
    Any other target returns `unknown-record` and changes nothing.
  - `in-force` to `revoked` needs a reason (at most 500 characters).
- A decision in a proposal is the "proposed" stage of the decision.
  The review flow replaces a status for it.

### Risk

- Fields: title (at most 200 characters), optional description (at most 4000), likelihood, impact, optional mitigation (at most 4000), owner, optional workstream and status.
- Likelihood and impact use three levels: `low`, `medium` and `high`.
  A separate change, the rating, changes them.
- Statuses: `open`, `mitigated`, `occurred` and `closed`.
- Transitions: `open` and `mitigated` change into each other.
  `open` and `mitigated` change to `occurred`.
  `open`, `mitigated` and `occurred` change to `closed`.
  `closed` changes to `open`.
- Lists sort by likelihood times impact (low 1, medium 2, high 3), then by impact, then by readable ID.

### Requirement

- Fields: text (at most 500 characters), optional source, owner, optional workstream, optional due date and status.
  The source is one person or one institution.
- Statuses: `open`, `met` and `waived`.
- Transitions: `open` and `met` change into each other.
  `open` changes to `waived` and needs a reason (at most 500 characters).
  `waived` is final.

### Operations in proposals

- `CreateMilestone` and `ChangeMilestoneStatus`.
- `CreateDecision`, `SupersedeDecision` (it carries the new decision) and `RevokeDecision` (it carries the reason).
- `CreateRisk`, `ChangeRiskStatus` and `ChangeRiskRating`.
- `CreateRequirement` and `ChangeRequirementStatus` (`waived` needs the reason).
- `CreateAction` gets the optional `start_date` and `milestone`.
  `ChangeActionMilestone` is new.
- `CreateWorkstream` carries the name and the lead (ADR 0080).
- The create operations carry the UUID of the new record.
  The change operations carry `expected_version`.

### Permissions and routing

- Who may do what in direct commands:
  - An event contributor or manager creates a milestone, a risk or a requirement.
  - The owner, the lead of its workstream or an event manager changes one of them.
  - Only an event manager creates, supersedes or revokes a decision, or changes its owner or workstream.
- Routing replaces the per-kind functions of ADR 0067 with two functions, the owner of a record and the lead of a workstream.
  Rules 1 to 3 of ADR 0067 apply to milestones, risks and requirements.
- Each decision operation, and `CreateWorkstream`, goes to the event managers.
  A contributor who leads a workstream does not review a decision in it.
- Rule 5 of ADR 0067 (the reviewers of a dependent also decide on the new record) gets these cases:
  - a new milestone that a new action or `ChangeActionMilestone` names;
  - a new person or institution that a new decision or requirement names;
  - a new workstream that a new record names.
    A new decision that `SupersedeDecision` names needs no case, because both go to the managers.
- The review edit (ADR 0068) cannot name a milestone of another event, or a canceled milestone, on a new action.

### Personal data

- `decision_party` and the source of a requirement refer to persons.
  The export of one person and the data inventory include them (ADR 0045).
- The legal redaction of an event clears the text of the four new kinds, and the quotes of their evidence, as it does for actions.
  The orphan sweep handles only blobs, so the new references do not affect it.

### MCP

- New read tools `list_milestones`, `list_decisions`, `list_risks` and `list_requirements` follow the form of `list_actions`.
- `get_event_schema` lists the four kinds with their statuses.
- `propose_changeset` accepts the new operations (ADR 0064).
- The tools always serialize `statuses`, also as an empty list (#73).
- A contract test validates the `structuredContent` of each tool against its `outputSchema`.
  It uses the crate `jsonschema`, a dev-dependency of the test crate only.
  The test checks the output against the schema that the server publishes, so a drift between the two fails the test.
  The product needs no new code or dependency for it.

### Timeline

- The web client shows one fixed, read-only timeline page for each event.
  It has no drag and no saved view.
  The page template is in the design system.
- The timeline reads the event window from the reserved core facts: `exact_dates` if accepted, else `date_window` if accepted or an assumption.
  If both are unknown, the timeline shows no band.

## Consequences

- A committee plans an event with milestones, decisions, risks and requirements.
  Acceptance criterion (12) becomes reachable without a separate task tool.
- Each decision stays a fixed record.
  A reader sees the wording that was in force, and the decision that replaced it.
- A new kind needs a table, a column in `record_evidence` and a case in the routing.
  The two functions keep the routing change small.
- Each new table joins the structured export, the data inventory and the isolation tests in the same commit.
- Decisions need an event manager for every change.
  A small club may find this strict.
  A later ADR can relax the rule.
- My Work, the AI PM digest and Telegram stay on actions and commitments in 2d.
  The event overview does not change.
  Follow-up issues cover both.

## Alternatives

- One generic record table with a JSON body (ADR 0049 and 0068): it moves the type checks from the schema to the code and weakens the foreign keys.
- Facts instead of records: a fact has no identity, owner or status, so it cannot carry a review, a due date or a transition.
- A many-to-many link between actions and milestones: it adds a link table and a harder warning rule.
  One milestone for each action matches how a committee plans.
- A five-level risk scale: the levels invite debate, and the data is too thin to tell them apart.
  Three levels are enough to sort.
- Route decisions like other records, to the owner and the workstream lead: a lead could then approve a decision that binds the whole event.
