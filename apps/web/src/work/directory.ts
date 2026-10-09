// What the work pages need to know about the event: the names of the members, the workstreams, and
// whether the member manages the event. The server decides each action; this only hides controls.
import { useCallback, useEffect, useMemo, useState } from "react";
import { type Api, type Problem, problemMessage, type Workstream } from "../api/client";
import { loadOrganizationMembers } from "../events/eventMembers";
import { canManage } from "../members/roles";
import { useSession } from "../session/SessionProvider";

export interface Choice {
  id: string;
  label: string;
}

/** The part of a record that decides who may change it. */
export interface Changeable {
  owner_user_id: string;
  workstream_id?: string | null | undefined;
}

export interface Directory {
  workstreams: Workstream[];
  /** True for an event manager and for an owner or admin of the organization. */
  isManager: boolean;
  nameOf: (userId: string) => string;
  /** The members that can own a record or lead a workstream. */
  assignees: Choice[];
  /** The owner, the lead of the workstream of the record, and an event manager may change it. */
  mayChange: (record: Changeable) => boolean;
}

export type DirectoryState =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; directory: Directory };

interface Loaded {
  names: Map<string, string>;
  workstreams: Workstream[];
  /** The event members who may own a record. Without the list of memberships: nothing. */
  contributors: Choice[] | undefined;
  eventManager: boolean;
}

function failed(error: Problem | undefined): DirectoryState {
  return { kind: "failed", message: problemMessage(error), requestId: error?.request_id };
}

/**
 * Loads the directory of an event. The list of event memberships is for event managers only, so
 * a list that loads shows a manager. Anyone else can own a record or lead a workstream only as
 * themselves in the forms.
 */
export function useDirectory(
  api: Api,
  eventId: string,
): { state: DirectoryState; reload: () => Promise<boolean> } {
  const session = useSession();
  const [loaded, setLoaded] = useState<Loaded>();
  const [error, setError] = useState<{ error: Problem | undefined }>();

  const reload = useCallback(async () => {
    try {
      const path = { event_id: eventId };
      const [members, workstreams, memberships] = await Promise.all([
        loadOrganizationMembers(api),
        api.GET("/api/v1/events/{event_id}/workstreams", { params: { path } }),
        api.GET("/api/v1/events/{event_id}/memberships", { params: { path } }),
      ]);
      if (!("members" in members) || !workstreams.data) {
        setError({ error: "members" in members ? workstreams.error : members.error });
        return false;
      }
      const names = new Map(members.members.map((m) => [m.user_id, m.display_name]));
      setLoaded({
        names,
        workstreams: workstreams.data.items,
        contributors: memberships.data?.items
          .filter((item) => item.event_role !== "event-viewer")
          .map((item) => ({
            id: item.user_id,
            label: names.get(item.user_id) ?? item.display_name,
          })),
        eventManager: memberships.data !== undefined,
      });
      setError(undefined);
      return true;
    } catch {
      setError({ error: undefined });
      return false;
    }
  }, [api, eventId]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const me = session.user.id;
  const organizationManager = canManage(session.organization?.role);
  const state = useMemo<DirectoryState>(() => {
    if (loaded) {
      const isManager = loaded.eventManager || organizationManager;
      const nameOf = (userId: string) => loaded.names.get(userId) ?? "";
      const leads = new Map(loaded.workstreams.map((w) => [w.id, w.lead_user_id]));
      return {
        kind: "loaded",
        directory: {
          workstreams: loaded.workstreams,
          isManager,
          nameOf,
          assignees: loaded.contributors ?? [
            { id: me, label: nameOf(me) || session.user.displayName },
          ],
          mayChange: (record) =>
            isManager ||
            record.owner_user_id === me ||
            (record.workstream_id != null && leads.get(record.workstream_id) === me),
        },
      };
    }
    return error ? failed(error.error) : { kind: "loading" };
  }, [loaded, error, me, organizationManager, session.user.displayName]);

  return { state, reload };
}
