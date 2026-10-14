import { describe, expect, it } from "vitest";
import { createApi } from "../api/client";
import { loadAll } from "../registers/loadAll";
import { json } from "../test/fakeWorkServer";
import { getRisk, listRisks } from "./risks";

const risk = (id: string) => ({ id, local_id: "RSK-001", title: "Regen" });

function serverWith(answer: (url: URL) => Response) {
  const calls: string[] = [];
  const fetch = async (request: Request) => {
    const url = new URL(request.url);
    calls.push(`${request.method} ${url.pathname}${url.search}`);
    return answer(url);
  };
  return { api: createApi(fetch as unknown as typeof globalThis.fetch), calls };
}

describe("risks", () => {
  it("lists the pages with the largest page size and the cursor", async () => {
    const { api, calls } = serverWith((url) =>
      url.searchParams.get("cursor") === "p2"
        ? json(200, { items: [risk("r2")] })
        : json(200, { items: [risk("r1")], next_cursor: "p2" }),
    );
    const result = await loadAll((cursor) => listRisks(api, "e1", cursor));
    expect(result).toEqual({ items: [risk("r1"), risk("r2")] });
    expect(calls).toEqual([
      "GET /api/v1/events/e1/risks?limit=200",
      "GET /api/v1/events/e1/risks?limit=200&cursor=p2",
    ]);
  });

  it("reads one risk", async () => {
    const { api, calls } = serverWith(() => json(200, risk("r1")));
    const { data } = await getRisk(api, "e1", "r1");
    expect(data).toEqual(risk("r1"));
    expect(calls).toEqual(["GET /api/v1/events/e1/risks/r1"]);
  });
});
