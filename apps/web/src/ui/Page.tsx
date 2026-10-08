import type { ReactNode, Ref } from "react";
import styles from "./Page.module.css";

export interface PageProps {
  /** `form` keeps the page in one column of at most 40rem, as the public page template needs. */
  width?: "full" | "form";
  children: ReactNode;
}

/**
 * The `main` element of a page template, with the page gutters of the app shell layouts
 * (doc/design/layout-and-responsiveness.md, „Page templates“). A page has exactly one.
 */
export function Page({ width = "full", children }: PageProps) {
  return (
    <main id="main" className={styles.page} data-width={width}>
      {children}
    </main>
  );
}

/** The `h1` of a page. Focus moves to it by script, for example after a route change. */
export function PageTitle({
  id,
  children,
  ref,
}: {
  /** For a section that the heading names with `aria-labelledby`. */
  id?: string;
  children: ReactNode;
  ref?: Ref<HTMLHeadingElement>;
}) {
  return (
    <h1 id={id} ref={ref} tabIndex={-1} className={styles.title}>
      {children}
    </h1>
  );
}
