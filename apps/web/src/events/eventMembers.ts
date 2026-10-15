// The rules of the event memberships screen that need no React.
import type { Api, EventMembership, EventRole, Problem } from "../api/client";
import { LARGEST_PAGE, loadAll } from "../registers/loadAll";

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
  const result = await loadAll((cursor) =>
    api.GET("/api/v1/members", {
      params: { query: { limit: LARGEST_PAGE, ...(cursor && { cursor }) } },
    }),
  );
  if ("error" in result) {
    return result;
  }
  return {
    members: result.items.map(({ user_id, display_name }) => ({ user_id, display_name })),
  };
}
