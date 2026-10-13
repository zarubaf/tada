// What the work pages need to know about the event: the names of the members, the workstreams, the
// milestones and the actions if the page asks for them, and whether the member manages the event.
// The server says per record what the caller may do (`can_change`); `isManager` serves only the
// workstreams, which have no such field yet.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  type Action,
  type Api,
  type Milestone,
  type Problem,
  problemMessage,
  type Workstream,
} from "../api/client";
import { loadOrganizationMembers } from "../events/eventMembers";
import { canManage } from "../members/roles";
import { LARGEST_PAGE, loadAll } from "../registers/loadAll";
import { useSession } from "../session/SessionProvider";

export interface Choice {
  id: string;
  label: string;
}

export interface Directory {
  workstreams: Workstream[];
  /** All milestones of the event, for a page that asked for them; otherwise empty. */
  milestones: Milestone[];
  /** All actions of the event, for a page that asked for them; otherwise empty. */
  actions: Action[];
  /** True for an event manager and for an owner or admin of the organization. */
  isManager: boolean;
  nameOf: (userId: string) => string;
  /** The members that can own a record or lead a workstream. */
  assignees: Choice[];
}

export type DirectoryState =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; directory: Directory };

interface Loaded {
  /** The event of the answer: a page that moved to another event must not show it. */
  eventId: string;
  names: Map<string, string>;
  workstreams: Workstream[];
  milestones: Milestone[];
  actions: Action[];
  /** The event members who may own a record or lead a workstream. */
  contributors: Choice[];
  eventManager: boolean;
}

function failed(error: Problem | undefined): DirectoryState {
  return { kind: "failed", message: problemMessage(error), requestId: error?.request_id };
}

/**
 * Loads the directory of an event: the members of the organization for the names, the workstreams
 * and the event memberships for the roles. Each member of the event reads the memberships. On
 * request, it also loads all milestones (for a form that links an action to one) or all actions
 * (for the milestones that count them).
 */
export function useDirectory(
  api: Api,
  eventId: string,
  {
    milestones: withMilestones = false,
    actions: withActions = false,
  }: { milestones?: boolean; actions?: boolean } = {},
): { state: DirectoryState; reload: () => Promise<boolean> } {
  const session = useSession();
  const [loaded, setLoaded] = useState<Loaded>();
  const [error, setError] = useState<{ eventId: string; error: Problem | undefined }>();
  // The newest request wins: the answer of an event that the page has left is dropped.
  const latest = useRef(0);

  const me = session.user.id;
  const reload = useCallback(async () => {
    const request = ++latest.current;
    const current = () => request === latest.current;
    try {
      const path = { event_id: eventId };
      const query = (cursor: string | undefined) => ({
        limit: LARGEST_PAGE,
        ...(cursor && { cursor }),
      });
      const [members, workstreams, memberships, milestones, actions] = await Promise.all([
        loadOrganizationMembers(api),
        api.GET("/api/v1/events/{event_id}/workstreams", { params: { path } }),
        api.GET("/api/v1/events/{event_id}/memberships", { params: { path } }),
        withMilestones
          ? loadAll((cursor) =>
              api.GET("/api/v1/events/{event_id}/milestones", {
                params: { path, query: query(cursor) },
              }),
            )
          : { items: [] },
        withActions
          ? loadAll((cursor) =>
              api.GET("/api/v1/events/{event_id}/actions", {
                params: { path, query: query(cursor) },
              }),
            )
          : { items: [] },
      ]);
      if (!current()) {
        return false;
      }
      if (!("members" in members)) {
        setError({ eventId, error: members.error });
        return false;
      }
      if (!workstreams.data || !memberships.data || "error" in milestones || "error" in actions) {
        const lists = "error" in milestones ? milestones.error : undefined;
        const failed = lists ?? ("error" in actions ? actions.error : undefined);
        setError({ eventId, error: workstreams.error ?? memberships.error ?? failed });
        return false;
      }
      const names = new Map(members.members.map((m) => [m.user_id, m.display_name]));
      setLoaded({
        eventId,
        names,
        workstreams: workstreams.data.items,
        milestones: milestones.items,
        actions: actions.items,
        contributors: memberships.data.items
          .filter((item) => item.event_role !== "event-viewer")
          .map((item) => ({
            id: item.user_id,
            label: names.get(item.user_id) ?? item.display_name,
          })),
        eventManager: memberships.data.items.some(
          (item) => item.user_id === me && item.event_role === "event-manager",
        ),
      });
      setError(undefined);
      return true;
    } catch {
      if (current()) {
        setError({ eventId, error: undefined });
      }
      return false;
    }
  }, [api, eventId, me, withMilestones, withActions]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const organizationManager = canManage(session.organization?.role);
  const state = useMemo<DirectoryState>(() => {
    if (loaded && loaded.eventId === eventId) {
      const isManager = loaded.eventManager || organizationManager;
      const nameOf = (userId: string) => loaded.names.get(userId) ?? "";
      return {
        kind: "loaded",
        directory: {
          workstreams: loaded.workstreams,
          milestones: loaded.milestones,
          actions: loaded.actions,
          isManager,
          nameOf,
          assignees: loaded.contributors,
        },
      };
    }
    return error && error.eventId === eventId ? failed(error.error) : { kind: "loading" };
  }, [loaded, error, eventId, organizationManager]);

  return { state, reload };
}
