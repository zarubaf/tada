import { describe, expect, it } from "vitest";
import { RECORD_KINDS } from "../recordStatus";
import { PANEL } from "../workLinks";
import { descriptorOf } from "./descriptors";
import { CREATABLE } from "./recordRoute";

describe("descriptors", () => {
  it("has a descriptor for exactly the kinds whose cards open the panel", () => {
    for (const kind of RECORD_KINDS) {
      expect([kind, descriptorOf(kind) !== undefined]).toEqual([kind, PANEL[kind]]);
    }
  });

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
      ].map((spec) => spec.id);
      expect(new Set(ids).size).toBe(ids.length);
      for (const id of descriptor.create?.fields ?? []) {
        expect(ids).toContain(id);
      }
    }
  });

  it("lists as creatable exactly the kinds whose descriptor can create", () => {
    const creating = RECORD_KINDS.filter((kind) => descriptorOf(kind)?.create !== undefined);
    expect([...CREATABLE].sort()).toEqual([...creating].sort());
  });
});
