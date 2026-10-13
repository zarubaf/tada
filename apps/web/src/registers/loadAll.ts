// All the pages of a list, for a page that needs every record of a kind, for example the milestones
// that an action can name or the actions that a milestone counts.
import type { Problem } from "../api/client";

type Answer<T> = Promise<{
  data?: { items: T[]; next_cursor?: string | null | undefined };
  error?: Problem | undefined;
}>;

/** The largest page that the server gives. */
export const LARGEST_PAGE = 200;

/** Loads the pages one after the other. `fetchPage` takes the cursor, or none for the first page. */
export async function loadAll<T>(
  fetchPage: (cursor: string | undefined) => Answer<T>,
): Promise<{ items: T[] } | { error: Problem | undefined }> {
  const items: T[] = [];
  let cursor: string | undefined;
  do {
    const { data, error } = await fetchPage(cursor);
    if (!data) {
      return { error };
    }
    items.push(...data.items);
    cursor = data.next_cursor ?? undefined;
  } while (cursor !== undefined);
  return { items };
}
