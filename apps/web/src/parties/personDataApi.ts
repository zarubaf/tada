// The calls of the data-protection section of a person (ADR 0073). Only an owner may make them.
import type { Api, Problem } from "../api/client";
import type { components } from "../api/schema.gen";

export type Mention = components["schemas"]["PersonMention"];

/** The kinds of mention that a redaction or a dispute note can target. Others edit normally. */
const TARGET_KINDS = ["source-item", "fact-version", "document-version", "proposal"];

/** The mention is selectable for a redaction or a dispute note. A redacted mention is not. */
export function isTarget(mention: Mention): boolean {
  return TARGET_KINDS.includes(mention.kind) && !mention.redacted;
}

/** The key that identifies a mention in a selection. */
export function mentionKey(mention: Mention): string {
  return `${mention.kind}:${mention.id}`;
}

/** The file name of the export from the `Content-Disposition` of the server, or a neutral name. */
export function exportFileName(response: Response): string {
  const disposition = response.headers.get("Content-Disposition") ?? "";
  return /filename="([^"]+)"/.exec(disposition)?.[1] ?? "person.json";
}

type Answer = { error?: Problem | undefined; response: Response };

export interface PersonDataApi {
  mentions(personId: string): Promise<Answer & { data?: { items: Mention[] } | undefined }>;
  /**
   * The export as a file. It is a call and not a plain link, because the server can ask for a new
   * sign-in first (`recent-sign-in-required`), and a link would save that problem as the file.
   */
  exportFile(personId: string): Promise<Answer & { data?: Blob | undefined }>;
  redact(targets: Mention[]): Promise<Answer>;
  dispute(target: Mention, text: string): Promise<Answer>;
}

export function personDataApi(api: Api): PersonDataApi {
  return {
    mentions: (personId) =>
      api.GET("/api/v1/persons/{person_id}/mentions", {
        params: { path: { person_id: personId } },
      }),
    exportFile: (personId) =>
      api.GET("/api/v1/persons/{person_id}/export", {
        params: { path: { person_id: personId } },
        parseAs: "blob",
      }),
    redact: (targets) =>
      api.POST("/api/v1/redactions", {
        body: { targets: targets.map(({ kind, id }) => ({ kind, id })), reason: "request" },
      }),
    dispute: (target, text) =>
      api.POST("/api/v1/dispute-notes", {
        body: { target_kind: target.kind, target_id: target.id, text },
      }),
  };
}
