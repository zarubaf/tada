import { describe, expect, it } from "vitest";
import { canManage, canRemove, invitableRoles } from "./roles";

describe("roles", () => {
  it("lets owners and admins manage", () => {
    expect(canManage("owner")).toBe(true);
    expect(canManage("admin")).toBe(true);
    expect(canManage("member")).toBe(false);
    expect(canManage(undefined)).toBe(false);
  });

  it("lets an owner invite with any role, an admin with admin or member, a member with none", () => {
    expect(invitableRoles("owner")).toEqual(["owner", "admin", "member"]);
    expect(invitableRoles("admin")).toEqual(["admin", "member"]);
    expect(invitableRoles("member")).toEqual([]);
    expect(invitableRoles(undefined)).toEqual([]);
  });

  it("removes members up to the own role only", () => {
    expect(canRemove("owner", "owner")).toBe(true);
    expect(canRemove("admin", "owner")).toBe(false);
    expect(canRemove("admin", "admin")).toBe(true);
    expect(canRemove("admin", "member")).toBe(true);
    expect(canRemove("member", "member")).toBe(false);
    expect(canRemove(undefined, "member")).toBe(false);
  });
});
