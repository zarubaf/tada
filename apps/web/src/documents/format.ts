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
