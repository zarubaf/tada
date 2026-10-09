// The rules of the members screen that need no React. The server decides each action; these rules
// only hide what would fail.
import type { OrganizationRole } from "../api/client";

/** The organization roles, most rights first. */
const ROLES: OrganizationRole[] = ["owner", "admin", "member"];

/** Owners and admins manage members and invitations. */
export function canManage(role: OrganizationRole | undefined): boolean {
  return role === "owner" || role === "admin";
}

/** An owner invites with any role; an admin with admin or member. */
export function invitableRoles(role: OrganizationRole | undefined): OrganizationRole[] {
  if (role === "owner") {
    return ROLES;
  }
  return role === "admin" ? ROLES.filter((r) => r !== "owner") : [];
}

/** A manager removes members, and revokes invitations, whose role is not higher than their own. */
export function canRemove(own: OrganizationRole | undefined, member: OrganizationRole): boolean {
  return own !== undefined && canManage(own) && ROLES.indexOf(member) >= ROLES.indexOf(own);
}
