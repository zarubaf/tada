import { describe, expect, it } from "vitest";
import type { EventMembership } from "../api/client";
import { addableMembers, canManageMembers } from "./eventMembers";

function membership(userId: string, role: EventMembership["event_role"]): EventMembership {
  return {
    user_id: userId,
    display_name: userId,
    event_role: role,
    version: 1,
    created_at: "2030-05-18T08:00:00Z",
  };
}

describe("canManageMembers", () => {
  const items = [membership("u1", "event-manager"), membership("u2", "event-contributor")];

  it("lets an organization owner or admin manage", () => {
    expect(canManageMembers("owner", "u9", items)).toBe(true);
    expect(canManageMembers("admin", "u9", items)).toBe(true);
  });

  it("lets an event manager manage", () => {
    expect(canManageMembers("member", "u1", items)).toBe(true);
  });

  it("does not let a contributor or an outsider manage", () => {
    expect(canManageMembers("member", "u2", items)).toBe(false);
    expect(canManageMembers("member", "u9", items)).toBe(false);
  });
});

describe("addableMembers", () => {
  it("leaves out the members that already have an event role", () => {
    const organization = [
      { user_id: "u1", display_name: "Anna Muster" },
      { user_id: "u2", display_name: "Bernd Beispiel" },
      { user_id: "u3", display_name: "Cäcilia Probst" },
    ];
    const items = [membership("u1", "event-manager"), membership("u3", "event-viewer")];
    expect(addableMembers(organization, items)).toEqual([
      { user_id: "u2", display_name: "Bernd Beispiel" },
    ]);
  });
});
