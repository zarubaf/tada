import { describe, expect, it } from "vitest";
import { RECORD_KINDS } from "./recordStatus";
import type { WorkItem } from "./workItem";
import { canEdit } from "./workPermissions";

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
