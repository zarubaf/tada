// The rules of the event memberships screen that need no React.
import type { Api, EventMembership, EventRole, Membership, Problem } from "../api/client";

/** The event roles in the order of their rights, most rights first. */
export const EVENT_ROLES: EventRole[] = ["event-manager", "event-contributor", "event-viewer"];

/** A member of the organization, as the picker needs it. */
export interface OrganizationMember {
  user_id: string;
  display_name: string;
}

/**
 * Whether the member manages the event memberships: an organization owner or admin acts as event
 * manager in each event (ADR 0052), and so does an event manager. The server decides anyway.
 */
export function canManageMembers(
  organizationRole: Membership["role"],
  userId: string,
  items: EventMembership[],
): boolean {
  return (
    organizationRole === "owner" ||
    organizationRole === "admin" ||
    items.some((item) => item.user_id === userId && item.event_role === "event-manager")
  );
}

/** The members of the organization that have no event role in the event yet. */
export function addableMembers(
  organization: OrganizationMember[],
  items: EventMembership[],
): OrganizationMember[] {
  const taken = new Set(items.map((item) => item.user_id));
  return organization.filter((member) => !taken.has(member.user_id));
}

// `GET /api/v1/members` (Task 13) is not in the contract yet. This is its shape, so that the
// screen works against it; use the generated client when the contract has the operation.
interface MembersPage {
  items: OrganizationMember[];
  next_cursor?: string | null;
}
type MembersClient = {
  GET(
    path: "/api/v1/members",
    init: { params: { query: { cursor?: string } } },
  ): Promise<{ data?: MembersPage; error?: Problem }>;
};

/** All members of the organization, page after page. */
export async function loadOrganizationMembers(
  api: Api,
): Promise<{ members: OrganizationMember[] } | { error: Problem | undefined }> {
  const client = api as unknown as MembersClient;
  const members: OrganizationMember[] = [];
  let cursor: string | undefined;
  do {
    const { data, error } = await client.GET("/api/v1/members", {
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
