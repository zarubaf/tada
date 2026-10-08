// Helpers of the tests of the pages that a member without a session can open.
import { screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import { createApi } from "../api/client";

export function json(
  status: number,
  body?: unknown,
  contentType = "application/json",
  headers: Record<string, string> = {},
) {
  return new Response(body === undefined ? null : JSON.stringify(body), {
    status,
    headers: { "Content-Type": contentType, ...headers },
  });
}

export function problem(status: number, code: string, headers: Record<string, string> = {}) {
  return json(
    status,
    {
      type: "",
      code,
      title: "",
      status,
      instance: "",
      request_id: "01a11165-c361-77e9-a636-584f1ee6643c",
    },
    "application/problem+json",
    headers,
  );
}

/** A fake server: each call returns the next response and records the call. */
export function fakeApi(...responses: Response[]) {
  const calls: { method: string; path: string; body: string }[] = [];
  const fetch = vi.fn(async (request: Request) => {
    calls.push({
      method: request.method,
      path: new URL(request.url).pathname,
      body: await request.clone().text(),
    });
    const response = responses.shift();
    if (!response) {
      throw new Error("no more responses");
    }
    return response;
  });
  return { api: createApi(fetch as unknown as typeof globalThis.fetch), calls };
}

/** The alerts that show a text. An empty live region is in the page before its text. */
function shownAlerts(): HTMLElement[] {
  return screen.queryAllByRole("alert").filter((alert) => alert.textContent?.trim());
}

/** The one alert that shows a text, or nothing. */
export function queryAlert(): HTMLElement | null {
  return shownAlerts()[0] ?? null;
}

/** Waits for the one alert that shows a text. */
export function findAlert(): Promise<HTMLElement> {
  return waitFor(() => {
    const shown = shownAlerts();
    if (shown.length !== 1) {
      throw new Error(`expected one alert with a text, found ${shown.length}`);
    }
    return shown[0] as HTMLElement;
  });
}
