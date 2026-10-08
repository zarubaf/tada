// The upload is the one call with a raw body: the file itself, with the media type
// `application/octet-stream`, not JSON and not a multipart form (ADR 0009, ADR 0043).
// The generated client types that body as a list of numbers and would serialize it as JSON, so this
// call turns the serializer off and passes the `File` through. It still goes through the client,
// so the session problems (ADR 0037) and the cookie rule apply to it as to every other call.
import { t } from "../i18n";
import type { Api, Document, Problem } from "./client";
import { failureOf } from "./failure";

export type UploadResult =
  | { document: Document }
  | { error: Problem | undefined; response?: Response };

/** Uploads `file` as the first version of a new document in the event. */
export async function uploadDocument(api: Api, eventId: string, file: File): Promise<UploadResult> {
  try {
    const { data, error, response } = await api.POST("/api/v1/events/{event_id}/documents", {
      params: {
        path: { event_id: eventId },
        // HTTP header values hold no raw non-ASCII text: the server decodes the percent-encoding.
        header: { "X-File-Name": encodeURIComponent(file.name) },
      },
      headers: { "Content-Type": "application/octet-stream" },
      body: file as unknown as number[],
      bodySerializer: (body) => body as unknown as BodyInit,
    });
    return data ? { document: data } : { error, response };
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
