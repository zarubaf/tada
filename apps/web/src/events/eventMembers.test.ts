import { describe, expect, it, vi } from "vitest";
import type { Api, EventMembership } from "../api/client";
import { addableMembers, loadOrganizationMembers } from "./eventMembers";

function membership(userId: string, role: EventMembership["event_role"]): EventMembership {
  return {
    user_id: userId,
    display_name: userId,
    event_role: role,
    version: 1,
    created_at: "2030-05-18T08:00:00Z",
  };
}

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

/** An API whose members come in pages; `pages[i]` answers the request with `cursors[i]`. */
function pagedApi(pages: { items: string[]; next: string | null }[]) {
  const queries: Record<string, unknown>[] = [];
  const GET = vi.fn(async (_path: string, init: { params: { query: Record<string, unknown> } }) => {
    queries.push(init.params.query);
    const page = pages[queries.length - 1];
    return {
      data: {
        items: (page?.items ?? []).map((id) => ({ user_id: id, display_name: id, role: "member" })),
        next_cursor: page?.next ?? null,
      },
    };
  });
  return { api: { GET } as unknown as Api, queries };
}

describe("loadOrganizationMembers", () => {
  it("asks for the largest page and follows the cursor", async () => {
    const { api, queries } = pagedApi([
      { items: ["u1"], next: "c1" },
      { items: ["u2"], next: null },
    ]);
    const result = await loadOrganizationMembers(api);

    expect(result).toEqual({
      members: [
        { user_id: "u1", display_name: "u1" },
        { user_id: "u2", display_name: "u2" },
      ],
    });
    expect(queries).toEqual([{ limit: 200 }, { limit: 200, cursor: "c1" }]);
  });

  it("stops when the server gives a cursor again", async () => {
    const { api, queries } = pagedApi([
      { items: ["u1"], next: "c1" },
      { items: ["u2"], next: "c1" },
      { items: ["u3"], next: "c1" },
    ]);
    const result = await loadOrganizationMembers(api);

    expect("members" in result && result.members.map((m) => m.user_id)).toEqual(["u1", "u2"]);
    expect(queries).toHaveLength(2);
  });
});
