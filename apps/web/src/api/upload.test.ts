import { describe, expect, it } from "vitest";
import { createApi, watchSessionProblems } from "./client";
import { uploadDocument, uploadMessage } from "./upload";

function problem(status: number, code: string, errors: unknown[] = []) {
  return new Response(
    JSON.stringify({ type: "", code, title: "", status, instance: "", request_id: "r1", errors }),
    { status, headers: { "Content-Type": "application/problem+json" } },
  );
}

describe("uploadDocument", () => {
  it("sends the raw file with the encoded file name", async () => {
    const seen: Request[] = [];
    const api = createApi((async (request: Request) => {
      seen.push(request);
      return new Response(JSON.stringify({ id: "d1" }), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      });
    }) as unknown as typeof globalThis.fetch);
    const file = new File(["inhalt"], "Übersicht 2030.txt", { type: "text/plain" });

    const result = await uploadDocument(api, "e1", file);

    expect(result).toEqual({ document: { id: "d1" } });
    const request = seen[0] as Request;
    expect(new URL(request.url).pathname).toBe("/api/v1/events/e1/documents");
    expect(request.method).toBe("POST");
    expect(request.headers.get("Content-Type")).toBe("application/octet-stream");
    expect(request.headers.get("X-File-Name")).toBe("%C3%9Cbersicht%202030.txt");
    expect(await request.text()).toBe("inhalt");
  });

  it("returns the problem of a failed upload", async () => {
    const api = createApi((async () => problem(413, "payload-too-large")) as never);
    const result = await uploadDocument(api, "e1", new File(["x"], "a.txt"));
    expect("error" in result && result.error?.code).toBe("payload-too-large");
  });

  it("reports an expired session like every other call", async () => {
    const api = createApi((async () => problem(401, "unauthenticated")) as never);
    const seen: string[] = [];
    api.use(watchSessionProblems((code) => seen.push(code)));

    await uploadDocument(api, "e1", new File(["x"], "a.txt"));

    expect(seen).toEqual(["unauthenticated"]);
  });

  it("returns no problem when the network fails", async () => {
    const api = createApi((async () => {
      throw new TypeError("network");
    }) as never);
    expect(await uploadDocument(api, "e1", new File(["x"], "a.txt"))).toEqual({ error: undefined });
  });
});

describe("uploadMessage", () => {
  const base = { type: "", title: "", instance: "", request_id: "r1" };

  it("names the size limit", () => {
    expect(uploadMessage({ ...base, code: "payload-too-large", status: 413 })).toBe(
      "Die Datei ist zu gross.",
    );
  });

  it("names the file type", () => {
    expect(uploadMessage({ ...base, code: "unsupported-media-type", status: 415 })).toBe(
      "Dieser Dateityp ist nicht erlaubt.",
    );
  });

  it("names the storage quota for the entry on /file", () => {
    expect(
      uploadMessage({
        ...base,
        code: "validation-failed",
        status: 422,
        errors: [{ pointer: "/file", code: "quota-exceeded" }],
      }),
    ).toBe("Der Speicherplatz der Organisation reicht für diese Datei nicht aus.");
  });

  it("falls back to the general message of the code", () => {
    expect(uploadMessage({ ...base, code: "validation-failed", status: 422, errors: [] })).toBe(
      "Die Eingabe ist ungültig.",
    );
  });
});
