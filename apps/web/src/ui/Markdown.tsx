import { type ComponentProps, type ReactNode, useMemo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import styles from "./Markdown.module.css";

/** The link schemes that survive (ADR 0058). Every other destination, also a relative one, is dropped. */
const ALLOWED_SCHEMES = new Set(["https", "mailto", "tada"]);

function schemeOf(url: string): string | undefined {
  return /^([a-z][a-z0-9+.-]*):/i.exec(url.trim())?.[1]?.toLowerCase();
}

function urlTransform(url: string): string {
  const scheme = schemeOf(url);
  return scheme && ALLOWED_SCHEMES.has(scheme) ? url : "";
}

/** A heading one level lower than in the text, because the page has the only h1. */
function heading(level: 2 | 3 | 4 | 5 | 6) {
  const Tag = `h${level}` as const;
  return function Heading({ children }: { children?: ReactNode }) {
    return <Tag>{children}</Tag>;
  };
}

/** A link that opens in a new tab without a referrer. A link without a safe destination shows its words. */
function externalLink({ href, children }: ComponentProps<"a">) {
  if (!href) {
    return <span>{children}</span>;
  }
  return (
    <a href={href} target="_blank" rel="noopener noreferrer">
      {children}
    </a>
  );
}

export interface MarkdownProps {
  children: string;
  /**
   * Renders a `tada:` link. It is no address: a view resolves it from data of the server
   * (ADR 0058). Without it, such a link shows its words. Keep the function stable, for example
   * with `useCallback`.
   */
  renderLink?: ((href: string, children: ReactNode) => ReactNode) | undefined;
}

const components: Components = {
  h1: heading(2),
  h2: heading(3),
  h3: heading(4),
  h4: heading(5),
  h5: heading(6),
  h6: heading(6),
  a: externalLink,
  // An image would load a remote address when a reader opens the text (ADR 0058).
  img({ alt }: ComponentProps<"img">) {
    return alt ? <span>{alt}</span> : null;
  },
  table({ children }: ComponentProps<"table">) {
    return (
      <div className={styles.scroll}>
        <table>{children}</table>
      </div>
    );
  },
};

/**
 * Safe Markdown (CommonMark with GitHub tables) for text that members or agents write (ADR 0058).
 * Raw HTML is dropped, images show their alternative text only, and the only links are `https`
 * and `mailto`, which open in a new tab without a referrer. Headings start at level 2.
 */
export function Markdown({ children, renderLink }: MarkdownProps) {
  const withLinks = useMemo(
    (): Components => ({
      ...components,
      a(props: ComponentProps<"a">) {
        const { href, children: words } = props;
        if (href && schemeOf(href) === "tada") {
          return renderLink ? renderLink(href, words) : <span>{words}</span>;
        }
        return externalLink(props);
      },
    }),
    [renderLink],
  );
  return (
    <div className={styles.markdown}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        urlTransform={urlTransform}
        components={withLinks}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
