// The rules of the event memberships screen that need no React.
import type { Api, EventMembership, EventRole, Problem } from "../api/client";

/** The event roles in the order of their rights, most rights first. */
export const EVENT_ROLES: EventRole[] = ["event-manager", "event-contributor", "event-viewer"];

/** A member of the organization, as the picker needs it. */
export interface OrganizationMember {
  user_id: string;
  display_name: string;
}

/** The members of the organization that have no event role in the event yet. */
export function addableMembers(
  organization: OrganizationMember[],
  items: EventMembership[],
): OrganizationMember[] {
  const taken = new Set(items.map((item) => item.user_id));
  return organization.filter((member) => !taken.has(member.user_id));
}

/** All members of the organization, page after page. */
export async function loadOrganizationMembers(
  api: Api,
): Promise<{ members: OrganizationMember[] } | { error: Problem | undefined }> {
  const members: OrganizationMember[] = [];
  let cursor: string | undefined;
  do {
    const { data, error } = await api.GET("/api/v1/members", {
      params: { query: cursor === undefined ? {} : { cursor } },
    });
    if (!data) {
      return { error };
    }
    members.push(...data.items.map(({ user_id, display_name }) => ({ user_id, display_name })));
    cursor = data.next_cursor ?? undefined;
  } while (cursor !== undefined);
  return { members };
}
