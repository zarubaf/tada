// User-facing text comes only from the Fluent files in the shared `locales/` folder (ADR 0005).
import { FluentBundle, FluentResource, type FluentVariable } from "@fluent/bundle";
import deCH from "../../../../locales/de-CH/web.ftl?raw";

export const LOCALE = "de-CH";

const bundle = new FluentBundle(LOCALE, { useIsolating: false });
const errors = bundle.addResource(new FluentResource(deCH));
if (errors.length > 0) {
  throw new Error(`invalid Fluent messages: ${errors.map(String).join("; ")}`);
}

/** Returns true if the locale has the message `id`. */
export function hasMessage(id: string): boolean {
  return bundle.hasMessage(id);
}

/** Formats the message `id`. A missing message shows its ID, so that the gap is visible. */
export function t(id: string, args?: Record<string, FluentVariable>): string {
  const pattern = bundle.getMessage(id)?.value;
  if (!pattern) {
    return id;
  }
  return bundle.formatPattern(pattern, args);
}
