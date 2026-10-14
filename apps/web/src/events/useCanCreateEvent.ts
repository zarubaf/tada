import { useOptionalSession } from "../session/SessionProvider";

/**
 * True if the member may create events: the organization owner and the administration. The server
 * decides; the controls only hide an action that would fail.
 */
export function useCanCreateEvent(): boolean {
  const role = useOptionalSession()?.organization?.role;
  return role === "owner" || role === "admin";
}
