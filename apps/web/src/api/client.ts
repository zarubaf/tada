// The web client calls the server only through this client, which the build generates from the
// contract (ADRs 0002 and 0017).
import createClient, { type Middleware } from "openapi-fetch";
import { hasMessage, t } from "../i18n";
import type { components, paths } from "./schema.gen";

export type Api = ReturnType<typeof createApi>;
export type Event = components["schemas"]["Event"];
export type Problem = components["schemas"]["Problem"];
export type SessionInfo = components["schemas"]["SessionInfo"];
export type Membership = components["schemas"]["MembershipSummary"];

/** The problem codes that the session handles for every call (ADR 0037). */
export const SESSION_PROBLEMS = ["unauthenticated", "organization-required"] as const;
export type SessionProblem = (typeof SESSION_PROBLEMS)[number];

export function createApi(fetch: typeof globalThis.fetch = globalThis.fetch) {
  const api = createClient<paths>({ baseUrl: globalThis.location?.origin ?? "", fetch });
  api.use(sameOriginCookies);
  return api;
}

/** The session cookie goes to the server of the page and nowhere else. */
const sameOriginCookies: Middleware = {
  onRequest: ({ request }) => new Request(request, { credentials: "same-origin" }),
};

/**
 * A middleware that reports the session problems of each response, whatever call got them.
 * The caller removes it with `api.eject`.
 */
export function watchSessionProblems(onProblem: (code: SessionProblem) => void): Middleware {
  return {
    onResponse: async ({ response }) => {
      if (response.ok || !response.headers.get("Content-Type")?.includes("problem+json")) {
        return;
      }
      const problem: Partial<Problem> = await response
        .clone()
        .json()
        .catch(() => ({}));
      const code = SESSION_PROBLEMS.find((known) => known === problem.code);
      if (code) {
        onProblem(code);
      }
    },
  };
}

/**
 * The German message of a failed request (ADR 0037): the message of the problem code if the locale
 * has one, otherwise the general message of the status class. Never the `detail` text.
 */
export function problemMessage(problem: Problem | undefined): string {
  if (!problem) {
    return t("problem-network");
  }
  const specific = `problem-${problem.code}`;
  if (hasMessage(specific)) {
    return t(specific);
  }
  return t(problem.status >= 500 ? "problem-general-5xx" : "problem-general-4xx");
}
