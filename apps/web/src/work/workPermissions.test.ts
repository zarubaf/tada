import { describe, expect, it } from "vitest";
import type { Directory } from "./directory";
import { RECORD_KINDS } from "./recordStatus";
import type { WorkItem } from "./workItem";
import { canCreate, canEdit } from "./workPermissions";

const item = (kind: WorkItem["kind"], over: { can_change?: boolean; redacted?: boolean } = {}) =>
  ({
    kind,
    redacted: over.redacted ?? false,
    record: { can_change: over.can_change ?? true },
  }) as unknown as WorkItem;

describe("canEdit", () => {
  it.each(RECORD_KINDS)("lets the member change a %s that the server allows", (kind) => {
    expect(canEdit(item(kind), true)).toBe(true);
  });

  it.each(RECORD_KINDS)("refuses a %s that the server does not allow", (kind) => {
    expect(canEdit(item(kind, { can_change: false }), true)).toBe(false);
  });

  it.each(RECORD_KINDS)("refuses a redacted %s", (kind) => {
    expect(canEdit(item(kind, { redacted: true }), true)).toBe(false);
  });

  it.each(RECORD_KINDS)("refuses a %s of a closed event", (kind) => {
    expect(canEdit(item(kind), false)).toBe(false);
  });
});

describe("canCreate", () => {
  const directory = (ids: string[], isManager = false) =>
    ({ isManager, assignees: ids.map((id) => ({ id, label: id })) }) as unknown as Directory;

  it("lets a contributor or manager of an open event create", () => {
    expect(canCreate(directory(["me", "other"]), "me", true)).toBe(true);
  });

  it("refuses a member who cannot own a record, such as a viewer", () => {
    expect(canCreate(directory(["other"]), "me", true)).toBe(false);
  });

  it("lets a manager create who is not a member of the event, as the server does", () => {
    // An owner or admin of the organization has manager level without a membership.
    expect(canCreate(directory(["other"], true), "me", true)).toBe(true);
  });

  it("refuses in a closed event", () => {
    expect(canCreate(directory(["me"]), "me", false)).toBe(false);
    expect(canCreate(directory(["me"], true), "me", false)).toBe(false);
  });

  it("refuses while the members are not known", () => {
    expect(canCreate(undefined, "me", true)).toBe(false);
  });
});
