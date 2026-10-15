import { Redirect, useHash, useParams, useSearch } from "../router/Router";

/**
 * The pages of an event that ADR 0082 replaced. Their old addresses stay in digests, Telegram
 * messages, bookmarks and the hash IDs of records, so each one leads to its new place.
 */
export const LEGACY_EVENT_PAGES = [
  "actions",
  "commitments",
  "milestones",
  "decisions",
  "timeline",
  "workstreams",
  "members",
] as const;
export type LegacyEventPage = (typeof LEGACY_EVENT_PAGES)[number];

/** Where the old page leads, below the address of the event: a sub-path, a query and a hash. */
const TARGETS: Record<LegacyEventPage, { path: string; set?: [string, string]; hash?: string }> = {
  actions: { path: "/work", set: ["type", "action"] },
  commitments: { path: "/work", set: ["type", "commitment"] },
  milestones: { path: "/work", set: ["type", "milestone"] },
  decisions: { path: "/work", set: ["type", "decision"] },
  timeline: { path: "/work", set: ["view", "timeline"] },
  // The workstreams are managed in a dialog of the work page.
  workstreams: { path: "/work", set: ["manage", "streams"] },
  // The members are a section of the overview now.
  members: { path: "", hash: "mitglieder" },
};

/**
 * A permanent redirect of a client-side route: it replaces the history entry, so that Back does
 * not return to the old address. It keeps the query keys it does not set and the hash of the
 * address, which names a record that the panel opens (`#ACT-001`).
 */
export function LegacyEventRedirect({ page }: { page: LegacyEventPage }) {
  const { eventId = "" } = useParams();
  const search = useSearch();
  const hash = useHash();
  const target = TARGETS[page];
  const params = new URLSearchParams(search.toString());
  if (target.set) {
    params.set(...target.set);
  }
  const query = params.toString();
  const fragment = target.hash ?? hash;
  return (
    <Redirect
      to={`/events/${encodeURIComponent(eventId)}${target.path}${query ? `?${query}` : ""}${
        fragment ? `#${encodeURIComponent(fragment)}` : ""
      }`}
    />
  );
}
