import { describe, expect, it } from "vitest";
import { loadAll } from "./loadAll";

const page = (items: number[], next_cursor?: string) =>
  Promise.resolve({ data: { items, next_cursor } });

describe("loadAll", () => {
  it("follows the cursors to the last page", async () => {
    const pages: Record<string, Promise<{ data: { items: number[]; next_cursor?: string } }>> = {
      first: page([1], "a"),
      a: page([2], "b"),
      b: page([3]),
    };
    const result = await loadAll((cursor) => pages[cursor ?? "first"] as never);
    expect(result).toEqual({ items: [1, 2, 3] });
  });

  it("ends the list when the server repeats a cursor", async () => {
    let calls = 0;
    const result = await loadAll(() => {
      calls += 1;
      return page([calls], "same");
    });
    expect(result).toEqual({ items: [1, 2] });
    expect(calls).toBe(2);
  });

  it("ends the list when the cursors run in a circle", async () => {
    const cursors = ["a", "b", "a", "b"];
    let calls = 0;
    const result = await loadAll(() => page([calls], cursors[calls++]));
    expect(calls).toBeLessThanOrEqual(3);
    expect("items" in result).toBe(true);
  });

  it("returns the error of a failed page", async () => {
    const error = { code: "x" } as never;
    const result = await loadAll(() => Promise.resolve({ error }));
    expect(result).toEqual({ error });
  });
});
