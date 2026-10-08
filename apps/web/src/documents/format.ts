// How the document screens show a file size and a hash.

import { LOCALE } from "../i18n";

const number = new Intl.NumberFormat(LOCALE, { maximumFractionDigits: 1 });
const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** The size with decimal units and at most one decimal place, for example „2.5 MB“. */
export function formatSize(bytes: number): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return `${number.format(value)} ${UNITS[unit]}`;
}

/** The first digits of a SHA-256 hash. A member compares them with the hash of a local file. */
export function hashPrefix(sha256: string): string {
  return sha256.slice(0, 12);
}

export type MediaTypeKind = "pdf" | "text" | "office" | "image" | "other";

const OFFICE_PREFIXES = [
  "application/vnd.openxmlformats-officedocument.",
  "application/vnd.oasis.opendocument.",
];
const TEXT_TYPES = ["text/plain", "text/markdown", "text/csv"];
const IMAGE_TYPES = ["image/png", "image/jpeg", "image/webp", "image/heic"];

/** The group of a media type among the upload types that the server allows (ADR 0055). */
export function mediaTypeKind(mediaType: string): MediaTypeKind {
  const type = (mediaType.split(";")[0] ?? "").trim().toLowerCase();
  if (type === "application/pdf") {
    return "pdf";
  }
  if (TEXT_TYPES.includes(type)) {
    return "text";
  }
  if (OFFICE_PREFIXES.some((prefix) => type.startsWith(prefix))) {
    return "office";
  }
  if (IMAGE_TYPES.includes(type)) {
    return "image";
  }
  return "other";
}
