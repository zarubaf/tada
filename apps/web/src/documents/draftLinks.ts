// What a draft view needs to know about the `tada:` links of a draft (ADR 0051, ADR 0058).

import type { LinkTarget } from "../api/client";

// The server accepts only the canonical form of a source link, without spaces (ADR 0058).
const SOURCE_LINK = /\]\((tada:source\/[^)\s]+)\)/g;

/**
 * The number of each cited source, by first appearance in the text. The same destination keeps its
 * number. A source that the reader cannot see gets none: it renders as „entfernt“.
 */
export function sourceNumbers(
  markdown: string,
  links: Record<string, LinkTarget>,
): Map<string, number> {
  const numbers = new Map<string, number>();
  for (const [, href] of markdown.matchAll(SOURCE_LINK)) {
    if (href && links[href]?.kind === "source" && !numbers.has(href)) {
      numbers.set(href, numbers.size + 1);
    }
  }
  return numbers;
}
