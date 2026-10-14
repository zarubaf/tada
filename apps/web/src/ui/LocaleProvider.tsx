import type { ReactNode } from "react";
import { I18nProvider } from "react-aria-components";

/**
 * Fixes the locale of React Aria (its own strings, dates and numbers). The client is German only,
 * so the language of the browser must not choose it: the build keeps the German strings only.
 */
export function LocaleProvider({ children }: { children: ReactNode }) {
  return <I18nProvider locale="de-CH">{children}</I18nProvider>;
}
