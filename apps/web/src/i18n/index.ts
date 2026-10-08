// User-facing text comes only from the Fluent files in the shared `locales/` folder (ADR 0005).
import { FluentBundle, FluentResource, type FluentVariable } from "@fluent/bundle";
import privacy from "../../../../locales/de-CH/privacy.ftl?raw";
import deCH from "../../../../locales/de-CH/web.ftl?raw";

export const LOCALE = "de-CH";

const bundle = new FluentBundle(LOCALE, { useIsolating: false });
const errors = [deCH, privacy].flatMap((source) => bundle.addResource(new FluentResource(source)));
if (errors.length > 0) {
  throw new Error(`invalid Fluent messages: ${errors.map(String).join("; ")}`);
}

/** Returns true if the locale has the message `id`. */
export function hasMessage(id: string): boolean {
  return bundle.hasMessage(id);
}

/**
 * The pseudo-locale of the layout test (ADR 0024): accented letters and 40 % more text.
 * Only builds that are not production builds have it, with `?pseudo` in the address.
 */
const pseudo =
  import.meta.env.MODE !== "production" &&
  new URLSearchParams(globalThis.location?.search ?? "").has("pseudo");

const ACCENTS: Record<string, string> = {
  a: "á",
  e: "é",
  i: "í",
  o: "ó",
  u: "ú",
  A: "Á",
  E: "É",
  I: "Í",
  O: "Ó",
  U: "Ú",
  c: "ç",
  n: "ñ",
};

export function pseudoLocalize(text: string): string {
  const accented = [...text].map((letter) => ACCENTS[letter] ?? letter).join("");
  const padding = "·".repeat(Math.ceil(text.length * 0.4));
  return `[${accented}${padding}]`;
}

/** Formats the message `id`. A missing message shows its ID, so that the gap is visible. */
export function t(id: string, args?: Record<string, FluentVariable>): string {
  const pattern = bundle.getMessage(id)?.value;
  if (!pattern) {
    return id;
  }
  const text = bundle.formatPattern(pattern, args);
  return pseudo ? pseudoLocalize(text) : text;
}
