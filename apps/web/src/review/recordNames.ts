// The names behind the IDs of a work proposal: the owner or lead, the workstream, the party of a
// commitment, the milestone of an action and the record that a change targets. The inbox looks
// them up once per changeset. A name that cannot be found is not an error: the view shows a
// neutral text.

import type { Api, Changeset } from "../api/client";
import { loadOrganizationMembers } from "../events/eventMembers";
import { milestoneName } from "../work/milestoneName";

export interface RecordNames {
  user: (id: string) => string | undefined;
  /** A workstream, new in the changeset or existing. */
  workstream: (id: string) => string | undefined;
  /** A person or an institution, new in the changeset or existing. */
  party: (id: string) => string | undefined;
  /**
   * An action, a commitment or a milestone: its readable ID and its text. A milestone that the
   * changeset creates has no readable ID yet: its title.
   */
  record: (id: string) => string | undefined;
}

export const NO_NAMES: RecordNames = {
  user: () => undefined,
  workstream: () => undefined,
  party: () => undefined,
  record: () => undefined,
};

/** The IDs that the proposals of the changeset point to, by kind of lookup. */
function references(changeset: Changeset) {
  const parties = new Set<string>();
  const actions = new Set<string>();
  const commitments = new Set<string>();
  const milestones = new Set<string>();
  let people = false;
  for (const { operation } of changeset.proposals) {
    if (
      operation.kind === "create-action" ||
      operation.kind === "create-commitment" ||
      operation.kind === "create-milestone" ||
      operation.kind === "create-workstream"
    ) {
      people = true;
    }
    if (operation.kind === "create-action" && operation.milestone) {
      milestones.add(operation.milestone);
    }
    if (operation.kind === "change-milestone-status") {
      milestones.add(operation.milestone_id);
    }
    if (operation.kind === "change-action-milestone") {
      actions.add(operation.action_id);
      if (operation.milestone) {
        milestones.add(operation.milestone);
      }
    }
    if (operation.kind === "create-commitment") {
      parties.add(
        "person" in operation.promisor ? operation.promisor.person : operation.promisor.institution,
      );
    }
    if (operation.kind === "change-action-status" || operation.kind === "change-action-due") {
      actions.add(operation.action_id);
    }
    if (operation.kind === "change-commitment-status") {
      commitments.add(operation.commitment_id);
    }
  }
  return { parties, actions, commitments, milestones, people };
}

/** Best effort: each lookup that fails leaves its names out. */
export async function loadRecordNames(api: Api, changeset: Changeset): Promise<RecordNames> {
  const eventId = changeset.event_id;
  const { parties, actions, commitments, milestones, people } = references(changeset);
  const users = new Map<string, string>();
  const workstreams = new Map<string, string>();
  const records = new Map<string, string>();
  const partyNames = new Map<string, string>();
  for (const { operation } of changeset.proposals) {
    if (operation.kind === "create-person" || operation.kind === "create-institution") {
      partyNames.set(operation.id, operation.name);
    }
    if (operation.kind === "create-workstream") {
      workstreams.set(operation.id, operation.name);
    }
    if (operation.kind === "create-milestone") {
      records.set(operation.id, operation.title);
    }
  }
  const attempt = async (work: () => Promise<void>) => {
    try {
      await work();
    } catch {
      // The view shows a neutral text for a name that is missing.
    }
  };
  const path = { event_id: eventId ?? "" };
  await Promise.all([
    ...(people && eventId
      ? [
          attempt(async () => {
            const members = await loadOrganizationMembers(api);
            for (const member of "members" in members ? members.members : []) {
              users.set(member.user_id, member.display_name);
            }
          }),
          attempt(async () => {
            const { data } = await api.GET("/api/v1/events/{event_id}/workstreams", {
              params: { path },
            });
            for (const item of data?.items ?? []) {
              if (!item.redacted) {
                workstreams.set(item.id, item.name);
              }
            }
          }),
        ]
      : []),
    ...[...parties]
      .filter((id) => !partyNames.has(id))
      .map((id) =>
        attempt(async () => {
          const person = await api.GET("/api/v1/persons/{person_id}", {
            params: { path: { person_id: id } },
          });
          if (person.data) {
            if (!person.data.redacted) {
              partyNames.set(id, person.data.name);
            }
            return;
          }
          const institution = await api.GET("/api/v1/institutions/{institution_id}", {
            params: { path: { institution_id: id } },
          });
          if (institution.data) {
            if (!institution.data.redacted) {
              partyNames.set(id, institution.data.name);
            }
          }
        }),
      ),
    ...[...actions].map((id) =>
      attempt(async () => {
        const { data } = await api.GET("/api/v1/events/{event_id}/actions/{id}", {
          params: { path: { ...path, id } },
        });
        if (data) {
          records.set(id, data.redacted ? data.local_id : `${data.local_id} ${data.title}`);
        }
      }),
    ),
    ...[...milestones]
      .filter((id) => !records.has(id))
      .map((id) =>
        attempt(async () => {
          const { data } = await api.GET("/api/v1/events/{event_id}/milestones/{id}", {
            params: { path: { ...path, id } },
          });
          if (data) {
            records.set(id, milestoneName(data));
          }
        }),
      ),
    ...[...commitments].map((id) =>
      attempt(async () => {
        const { data } = await api.GET("/api/v1/events/{event_id}/commitments/{id}", {
          params: { path: { ...path, id } },
        });
        if (data) {
          records.set(id, data.redacted ? data.local_id : `${data.local_id} ${data.text}`);
        }
      }),
    ),
  ]);
  return {
    user: (id) => users.get(id),
    workstream: (id) => workstreams.get(id),
    party: (id) => partyNames.get(id),
    record: (id) => records.get(id),
  };
}
