// The only call to the server that does not go through the generated client.
// The upload body is the raw file with the media type `application/octet-stream`, not JSON and not
// a multipart form (ADR 0009, ADR 0043). The client that the build generates types a binary body
// as a list of numbers and serializes it as JSON, so it cannot send a file. A `fetch` with a
// `File` body streams the file from the disk and sets `Content-Length`.
import { t } from "../i18n";
import type { Document, Problem } from "./client";
import { failureOf } from "./failure";

export type UploadResult =
  | { document: Document }
  | { error: Problem | undefined; response?: Response };

/** Uploads `file` as the first version of a new document in the event. */
export async function uploadDocument(eventId: string, file: File): Promise<UploadResult> {
  try {
    const response = await fetch(`/api/v1/events/${encodeURIComponent(eventId)}/documents`, {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/octet-stream",
        // HTTP header values hold no raw non-ASCII text: the server decodes the percent-encoding.
        "X-File-Name": encodeURIComponent(file.name),
      },
      body: file,
    });
    if (response.ok) {
      return { document: (await response.json()) as Document };
    }
    const error = response.headers.get("Content-Type")?.includes("problem+json")
      ? ((await response.json().catch(() => undefined)) as Problem | undefined)
      : undefined;
    return { error, response };
  } catch {
    return { error: undefined };
  }
}

/**
 * The German message of a failed upload (ADR 0037). A full storage quota is a `validation-failed`
 * entry on `/file`; the size and the type have their own codes.
 */
export function uploadMessage(error: Problem | undefined, response?: Response): string {
  const overQuota = error?.errors?.some(
    (entry) => entry.pointer === "/file" && entry.code === "quota-exceeded",
  );
  return overQuota
    ? t("document-error-file-quota-exceeded")
    : failureOf({ error, response }).message;
}
