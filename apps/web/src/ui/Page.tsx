import type { ReactNode, Ref } from "react";
import styles from "./Page.module.css";

export interface PageProps {
  /**
   * `form` keeps the page in one column of at most 40rem, as the public page template needs.
   * `flush` has no gutter of its own: the page template sets `--page-gutter` for its rows and
   * keeps its toolbar edge to edge, as the „Arbeit“ page does.
   */
  width?: "full" | "form" | "flush";
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

/**
 * The `h1` of a page. Focus moves to it by script, for example after a route change. A `compact`
 * title is the size of the interface text, for a title that sits in a toolbar row.
 */
export function PageTitle({
  id,
  children,
  compact,
  wrap,
  ref,
}: {
  /** For a section that the heading names with `aria-labelledby`. */
  id?: string;
  children: ReactNode;
  compact?: boolean;
  /**
   * A compact title may wrap, for a title that is a name of any length, for example of a document.
   * It then shrinks in the row and breaks inside a long word.
   */
  wrap?: boolean;
  ref?: Ref<HTMLHeadingElement>;
}) {
  return (
    <h1
      id={id}
      ref={ref}
      tabIndex={-1}
      className={styles.title}
      {...(compact && { "data-compact": "" })}
      {...(compact && wrap && { "data-wrap": "" })}
    >
      {children}
    </h1>
  );
}

/**
 * The title of a panel in a page with tabs: an `h2` below the `h1` of the page. Focus moves to it
 * by script, as to a page title, for example after a retry.
 */
export function PanelTitle({
  id,
  children,
  ref,
}: {
  id?: string;
  children: ReactNode;
  ref?: Ref<HTMLHeadingElement>;
}) {
  return (
    <h2 id={id} ref={ref} tabIndex={-1} className={styles.panelTitle}>
      {children}
    </h2>
  );
}
