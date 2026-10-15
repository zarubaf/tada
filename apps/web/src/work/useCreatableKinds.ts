// The kinds of record that the member may create in an event, for a place that has no work page
// around it, such as the command palette. The page itself reads the whole directory.
import { useEffect, useState } from "react";
import type { Api, RecordKind } from "../api/client";
import { canManage } from "../members/roles";
import { useSession } from "../session/SessionProvider";
import { isContributor, isEventManager, type Memberships } from "./directory";
import { creatableKinds } from "./workPermissions";

/**
 * The kinds that the member may create in the event, in the order of the „Neu“ menu; none while
 * they are not known. An owner or admin of the organization manages every event, so that member
 * asks nothing. Without an event, there is nothing to create. Everybody else asks for the memberships of the event and nothing more.
 */
export function useCreatableKinds(
  api: Api,
  eventId: string | undefined,
  eventWritable: boolean,
): readonly RecordKind[] {
  const session = useSession();
  const me = session.user.id;
  const managesAll = canManage(session.organization?.role);
  const [read, setRead] = useState<{ eventId: string; items: Memberships }>();

  useEffect(() => {
    if (managesAll || eventId === undefined) {
      return;
    }
    let current = true;
    api
      .GET("/api/v1/events/{event_id}/memberships", { params: { path: { event_id: eventId } } })
      .then(({ data }) => {
        if (current && data) {
          setRead({ eventId, items: data.items });
        }
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [api, eventId, managesAll]);

  if (eventId === undefined) {
    return [];
  }
  if (managesAll) {
    return creatableKinds({ assignees: [], isManager: true }, me, eventWritable);
  }
  if (read?.eventId !== eventId) {
    return [];
  }
  const { items } = read;
  const assignees = items.filter(isContributor).map((item) => ({ id: item.user_id }));
  return creatableKinds({ assignees, isManager: isEventManager(items, me) }, me, eventWritable);
}
