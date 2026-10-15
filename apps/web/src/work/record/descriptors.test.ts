import { describe, expect, it } from "vitest";
import { commitment } from "../../test/workRecords";
import { RECORD_KINDS } from "../recordStatus";
import { toWorkItem } from "../workItem";
import { descriptorOf } from "./descriptors";
import { CREATABLE } from "./recordRoute";

describe("descriptors", () => {
  it("gives each descriptor its own kind and unique field IDs", () => {
    for (const kind of RECORD_KINDS) {
      const descriptor = descriptorOf(kind);
      if (!descriptor) {
        continue;
      }
      expect(descriptor.kind).toBe(kind);
      const ids = [
        descriptor.title,
        ...descriptor.properties,
        ...(descriptor.description ? [descriptor.description] : []),
        ...(descriptor.texts ?? []),
      ].map((spec) => spec.id);
      expect(new Set(ids).size).toBe(ids.length);
      for (const id of descriptor.create?.fields ?? []) {
        expect(ids).toContain(id);
      }
    }
  });

  it("gives the actions of a descriptor unique IDs", () => {
    for (const kind of RECORD_KINDS) {
      const ids = (descriptorOf(kind)?.actions ?? []).map((action) => action.id);
      expect(new Set(ids).size).toBe(ids.length);
    }
  });

  it("never offers firm as a choice of a commitment status", () => {
    const status = descriptorOf("commitment")?.properties.find((spec) => spec.id === "status");
    const item = toWorkItem(
      "commitment",
      commitment("COM-001", { next_statuses: ["firm", "fulfilled"] }) as never,
      () => "",
    );
    expect(status?.options?.(item)).toEqual(["conditional", "fulfilled"]);
  });

  it("lists as creatable exactly the kinds whose descriptor can create", () => {
    const creating = RECORD_KINDS.filter((kind) => descriptorOf(kind)?.create !== undefined);
    expect([...CREATABLE].sort()).toEqual([...creating].sort());
  });
});
