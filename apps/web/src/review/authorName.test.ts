import { describe, expect, it } from "vitest";
import type { Author } from "../api/client";
import { authorName } from "./authorName";

const base = { id: "0199b8e0-0000-7000-8000-0000000000c1", channel: "web" } as const;

describe("authorName", () => {
  it("names a member and an AI client", () => {
    expect(authorName({ ...base, kind: "member" })).toBe("Mitglied");
    expect(authorName({ ...base, kind: "ai" })).toBe("KI-Client");
  });

  it("falls back for an author kind that this client does not know", () => {
    expect(authorName({ ...base, kind: "robot" } as unknown as Author)).toBe(
      "Unbekannter Absender",
    );
  });
});
