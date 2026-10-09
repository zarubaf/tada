import { describe, expect, it } from "vitest";
import type { DocumentVersion } from "../api/client";
import { diffPath, isApprovable, newestVersion, previousDraft } from "./versions";

function version(
  number: number,
  kind: "upload" | "draft",
  status?: DocumentVersion["status"],
): DocumentVersion {
  return {
    id: `v${number}`,
    document_id: "d1",
    number,
    kind,
    ...(status && { status }),
    sha256: "0",
    uploaded_by: "u1",
    created_at: "2030-05-18T08:00:00Z",
  };
}

const versions = [
  version(3, "draft", "review"),
  version(1, "upload"),
  version(2, "draft", "superseded"),
];

describe("newestVersion", () => {
  it("is the version with the highest number, whatever the order", () => {
    expect(newestVersion(versions)?.number).toBe(3);
    expect(newestVersion([])).toBeUndefined();
  });
});

describe("isApprovable", () => {
  it.each([
    [version(3, "draft", "draft"), true],
    [version(3, "draft", "review"), true],
    [version(3, "draft", "approved"), false],
    [version(3, "draft", "superseded"), false],
    [version(3, "draft", "archived"), false],
    [version(3, "upload"), false],
  ])("for %j is %s", (candidate, expected) => {
    expect(isApprovable(candidate)).toBe(expected);
  });
});

describe("previousDraft", () => {
  it("is the nearest older draft, never an upload", () => {
    expect(previousDraft(versions, version(3, "draft"))?.number).toBe(2);
    expect(previousDraft(versions, version(2, "draft"))).toBeUndefined();
  });
});

describe("diffPath", () => {
  it("names the document and both versions", () => {
    expect(diffPath("d1", version(2, "draft"), version(3, "draft"))).toBe(
      "/documents/d1/diff?from=v2&to=v3",
    );
  });
});
