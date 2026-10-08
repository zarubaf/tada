import { afterEach, describe, expect, it, vi } from "vitest";
import { uploadDocument, uploadMessage } from "./upload";

function problem(status: number, code: string, errors: unknown[] = []) {
  return new Response(
    JSON.stringify({ type: "", code, title: "", status, instance: "", request_id: "r1", errors }),
    { status, headers: { "Content-Type": "application/problem+json" } },
  );
}

afterEach(() => vi.unstubAllGlobals());

describe("uploadDocument", () => {
  it("sends the raw file with the encoded file name", async () => {
    const fetch = vi.fn(async () => new Response(JSON.stringify({ id: "d1" }), { status: 201 }));
    vi.stubGlobal("fetch", fetch);
    const file = new File(["inhalt"], "Übersicht 2030.txt", { type: "text/plain" });

    const result = await uploadDocument("e1", file);

    expect(result).toEqual({ document: { id: "d1" } });
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/v1/events/e1/documents");
    expect(init.method).toBe("POST");
    expect(init.body).toBe(file);
    expect(init.credentials).toBe("same-origin");
    const headers = new Headers(init.headers);
    expect(headers.get("Content-Type")).toBe("application/octet-stream");
    expect(headers.get("X-File-Name")).toBe("%C3%9Cbersicht%202030.txt");
  });

  it("returns the problem of a failed upload", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => problem(413, "payload-too-large")),
    );
    const result = await uploadDocument("e1", new File(["x"], "a.txt"));
    expect("error" in result && result.error?.code).toBe("payload-too-large");
  });

  it("returns no problem when the network fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new TypeError("network");
      }),
    );
    expect(await uploadDocument("e1", new File(["x"], "a.txt"))).toEqual({ error: undefined });
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
