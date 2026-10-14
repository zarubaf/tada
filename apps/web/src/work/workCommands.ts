// The commands that change a work record, one set per kind, over the endpoints that exist today.
// The side panel, the board menu and the bulk action call these and need no rule of their own.
//
// A kind has a command only if the API offers it, so a missing command is a type error and never a
// failure at run time. What the API does NOT offer (checked against the contract):
// - decision: no `changeStatus` (the status changes through supersede and revoke, which take a
//   reason or a successor), no `changeDue` (the day of a decision never changes) and no `rename`
//   (the wording never changes).
// - risk: no `changeDue` (a risk has no date).
// - commitment: `changeStatus` cannot set `firm` (only "make firm" does), and the condition never
//   changes.
// - milestone: `changeDue` sets the date, which cannot be cleared.
// A milestone, risk or decision has no `start_date`.
//
// Commands on one record run one after the other and each takes the version from the answer of the
// one before; commands on different records run at the same time.
import type {
  Action,
  ActionStatus,
  Api,
  ChangeActionRequest,
  ChangeCommitmentRequest,
  ChangeDecisionRequest,
  ChangeMilestoneRequest,
  ChangeRiskRequest,
  Commitment,
  CommitmentStatus,
  CreateActionRequest,
  Decision,
  Milestone,
  MilestoneStatus,
  Problem,
  Risk,
  RiskStatus,
} from "../api/client";
import { saveFailure } from "../registers/saveFailure";

type Body<T> = Omit<T, "expected_version">;

/** Why a command failed, for the code that decides what to show. */
export type FailureReason = "conflict" | "forbidden" | "not-found" | "validation" | "other";

export interface WorkFailure {
  reason: FailureReason;
  /** The German message (ADR 0037). */
  message: string;
  /** The problem of the server, for a form that maps its invalid fields. */
  problem?: Problem | undefined;
}

export type CommandResult<R> = { ok: true; record: R } | { ok: false; failure: WorkFailure };

/** What a command needs of a record: which one, and the version the caller last saw. */
export interface Target {
  id: string;
  version: number;
}

function reasonOf(error: Problem | undefined): FailureReason {
  switch (error?.code) {
    case "record-version-conflict":
      return "conflict";
    case "forbidden":
      return "forbidden";
    case "not-found":
      return "not-found";
    case "validation-failed":
      return "validation";
    default:
      return "other";
  }
}

type Answer<R> = Promise<{
  data?: R;
  error?: Problem | undefined;
  response?: Response | undefined;
}>;

/** The failure of an answer without data. */
function refused(answer: Awaited<Answer<unknown>>): { ok: false; failure: WorkFailure } {
  const { message } = saveFailure(answer);
  return { ok: false, failure: { reason: reasonOf(answer.error), message, problem: answer.error } };
}

/** Runs the commands of each record one after the other, with the newest version it knows. */
class RecordQueue {
  private tails = new Map<string, Promise<unknown>>();
  private versions = new Map<string, number>();

  run<R extends Target>(
    target: Target,
    send: (version: number) => Answer<R>,
  ): Promise<CommandResult<R>> {
    const step = async (): Promise<CommandResult<R>> => {
      // The answer of the command before knows a newer version than the caller does.
      const version = Math.max(target.version, this.versions.get(target.id) ?? 0);
      let answer: Awaited<Answer<R>>;
      try {
        answer = await send(version);
      } catch {
        answer = {};
      }
      if (!answer.data) {
        const result = refused(answer);
        // Only a conflict or a missing record proves the remembered version wrong; a refused value,
        // a refusal or a lost connection leaves the record as the last answer left it.
        if (result.failure.reason === "conflict" || result.failure.reason === "not-found") {
          this.versions.delete(target.id);
        }
        return result;
      }
      this.versions.set(target.id, answer.data.version);
      return { ok: true, record: answer.data };
    };
    const result = (this.tails.get(target.id) ?? Promise.resolve()).then(step);
    const tail = result.finally(() => {
      if (this.tails.get(target.id) === tail) {
        this.tails.delete(target.id);
        this.versions.delete(target.id);
      }
    });
    this.tails.set(target.id, tail);
    return result;
  }
}

export function createWorkCommands(api: Api, eventId: string) {
  const queue = new RecordQueue();
  const path = (id: string) => ({ event_id: eventId, id });

  const action = (target: Target, body: Body<ChangeActionRequest>) =>
    queue.run<Action>(target, (expected_version) =>
      api.PATCH("/api/v1/events/{event_id}/actions/{id}", {
        params: { path: path(target.id) },
        body: { ...body, expected_version },
      }),
    );
  const commitment = (target: Target, body: Body<ChangeCommitmentRequest>) =>
    queue.run<Commitment>(target, (expected_version) =>
      api.PATCH("/api/v1/events/{event_id}/commitments/{id}", {
        params: { path: path(target.id) },
        body: { ...body, expected_version },
      }),
    );
  const milestone = (target: Target, body: Body<ChangeMilestoneRequest>) =>
    queue.run<Milestone>(target, (expected_version) =>
      api.PATCH("/api/v1/events/{event_id}/milestones/{id}", {
        params: { path: path(target.id) },
        body: { ...body, expected_version },
      }),
    );
  const decision = (target: Target, body: Body<ChangeDecisionRequest>) =>
    queue.run<Decision>(target, (expected_version) =>
      api.PATCH("/api/v1/events/{event_id}/decisions/{id}", {
        params: { path: path(target.id) },
        body: { ...body, expected_version },
      }),
    );
  const risk = (target: Target, body: Body<ChangeRiskRequest>) =>
    queue.run<Risk>(target, (expected_version) =>
      api.PATCH("/api/v1/events/{event_id}/risks/{id}", {
        params: { path: path(target.id) },
        body: { ...body, expected_version },
      }),
    );

  /** A new record has no version to guard and no commands before it, so it needs no queue. */
  const createAction = async (body: CreateActionRequest): Promise<CommandResult<Action>> => {
    let answer: Awaited<Answer<Action>>;
    try {
      answer = await api.POST("/api/v1/events/{event_id}/actions", {
        params: { path: { event_id: eventId } },
        body,
      });
    } catch {
      answer = {};
    }
    return answer.data ? { ok: true, record: answer.data } : refused(answer);
  };

  return {
    action: {
      create: createAction,
      changeStatus: (t: Target, status: ActionStatus) => action(t, { status }),
      changeOwner: (t: Target, userId: string) => action(t, { owner_user_id: userId }),
      changeDue: (t: Target, due: string | null) => action(t, { due_date: due }),
      changeStart: (t: Target, start: string | null) => action(t, { start_date: start }),
      changeWorkstream: (t: Target, id: string | null) => action(t, { workstream_id: id }),
      changeMilestone: (t: Target, id: string | null) => action(t, { milestone_id: id }),
      changeDescription: (t: Target, text: string | null) => action(t, { description: text }),
      rename: (t: Target, title: string) => action(t, { title }),
    },
    commitment: {
      /** Not `firm`: only "make firm" sets it. */
      changeStatus: (t: Target, status: Exclude<CommitmentStatus, "firm">) =>
        commitment(t, { status }),
      changeOwner: (t: Target, userId: string) => commitment(t, { owner_user_id: userId }),
      changeDue: (t: Target, due: string | null) => commitment(t, { due_date: due }),
      changeWorkstream: (t: Target, id: string | null) => commitment(t, { workstream_id: id }),
      rename: (t: Target, text: string) => commitment(t, { text }),
    },
    milestone: {
      changeStatus: (t: Target, status: MilestoneStatus) => milestone(t, { status }),
      changeOwner: (t: Target, userId: string) => milestone(t, { owner_user_id: userId }),
      /** The date of a milestone cannot be cleared. */
      changeDue: (t: Target, date: string) => milestone(t, { date }),
      changeWorkstream: (t: Target, id: string | null) => milestone(t, { workstream_id: id }),
      rename: (t: Target, title: string) => milestone(t, { title }),
    },
    decision: {
      changeOwner: (t: Target, userId: string) => decision(t, { owner_user_id: userId }),
      changeWorkstream: (t: Target, id: string | null) => decision(t, { workstream_id: id }),
    },
    risk: {
      changeStatus: (t: Target, status: RiskStatus) => risk(t, { status }),
      changeOwner: (t: Target, userId: string) => risk(t, { owner_user_id: userId }),
      changeWorkstream: (t: Target, id: string | null) => risk(t, { workstream_id: id }),
      rename: (t: Target, title: string) => risk(t, { title }),
    },
  };
}

export type WorkCommands = ReturnType<typeof createWorkCommands>;
