// What the work pages need to know about the event: the names of the members, the workstreams, and
// whether the member manages the event. The server says per record what the caller may do
// (`can_change`); `isManager` serves only the workstreams, which have no such field yet.
import { useCallback, useEffect, useMemo, useState } from "react";
import { type Api, type Problem, problemMessage, type Workstream } from "../api/client";
import { loadOrganizationMembers } from "../events/eventMembers";
import { canManage } from "../members/roles";
import { useSession } from "../session/SessionProvider";

export interface Choice {
  id: string;
  label: string;
}

export interface Directory {
  workstreams: Workstream[];
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
  names: Map<string, string>;
  workstreams: Workstream[];
  /** The event members who may own a record or lead a workstream. */
  contributors: Choice[];
  eventManager: boolean;
}

function failed(error: Problem | undefined): DirectoryState {
  return { kind: "failed", message: problemMessage(error), requestId: error?.request_id };
}

/**
 * Loads the directory of an event: the members of the organization for the names, the workstreams
 * and the event memberships for the roles. Each member of the event reads the memberships.
 */
export function useDirectory(
  api: Api,
  eventId: string,
): { state: DirectoryState; reload: () => Promise<boolean> } {
  const session = useSession();
  const [loaded, setLoaded] = useState<Loaded>();
  const [error, setError] = useState<{ error: Problem | undefined }>();

  const me = session.user.id;
  const reload = useCallback(async () => {
    try {
      const path = { event_id: eventId };
      const [members, workstreams, memberships] = await Promise.all([
        loadOrganizationMembers(api),
        api.GET("/api/v1/events/{event_id}/workstreams", { params: { path } }),
        api.GET("/api/v1/events/{event_id}/memberships", { params: { path } }),
      ]);
      if (!("members" in members) || !workstreams.data || !memberships.data) {
        setError({
          error: "members" in members ? (workstreams.error ?? memberships.error) : members.error,
        });
        return false;
      }
      const names = new Map(members.members.map((m) => [m.user_id, m.display_name]));
      setLoaded({
        names,
        workstreams: workstreams.data.items,
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
      setError({ error: undefined });
      return false;
    }
  }, [api, eventId, me]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const organizationManager = canManage(session.organization?.role);
  const state = useMemo<DirectoryState>(() => {
    if (loaded) {
      const isManager = loaded.eventManager || organizationManager;
      const nameOf = (userId: string) => loaded.names.get(userId) ?? "";
      return {
        kind: "loaded",
        directory: {
          workstreams: loaded.workstreams,
          isManager,
          nameOf,
          assignees: loaded.contributors,
        },
      };
    }
    return error ? failed(error.error) : { kind: "loading" };
  }, [loaded, error, organizationManager]);

  return { state, reload };
}
