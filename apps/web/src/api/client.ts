// The web client calls the server only through this client, which the build generates from the
// contract (ADRs 0002 and 0017).
import createClient from "openapi-fetch";
import { hasMessage, t } from "../i18n";
import type { components, paths } from "./schema.gen";

export type Api = ReturnType<typeof createApi>;
export type Event = components["schemas"]["Event"];
export type Problem = components["schemas"]["Problem"];

export function createApi(fetch: typeof globalThis.fetch = globalThis.fetch) {
  return createClient<paths>({ baseUrl: globalThis.location?.origin ?? "", fetch });
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
