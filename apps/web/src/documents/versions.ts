// Rules about the versions of one document that need no React (ADR 0051).

import type { DocumentVersion } from "../api/client";

/** The version with the highest number. */
export function newestVersion(versions: DocumentVersion[]): DocumentVersion | undefined {
  return versions.reduce<DocumentVersion | undefined>(
    (newest, version) => (!newest || version.number > newest.number ? version : newest),
    undefined,
  );
}

/**
 * True for a draft that can still be approved. The server decides at the call: it also refuses a
 * draft that is older than the approved version.
 */
export function isApprovable(version: DocumentVersion): boolean {
  return version.kind === "draft" && (version.status === "draft" || version.status === "review");
}

/** The nearest older draft of `version`: the difference compares two drafts. */
export function previousDraft(
  versions: DocumentVersion[],
  version: DocumentVersion,
): DocumentVersion | undefined {
  return versions
    .filter((candidate) => candidate.kind === "draft" && candidate.number < version.number)
    .sort((a, b) => b.number - a.number)[0];
}

/** The address of the comparison of two draft versions. */
export function diffPath(documentId: string, from: DocumentVersion, to: DocumentVersion): string {
  const query = new URLSearchParams({ from: from.id, to: to.id });
  return `/documents/${encodeURIComponent(documentId)}/diff?${query}`;
}
