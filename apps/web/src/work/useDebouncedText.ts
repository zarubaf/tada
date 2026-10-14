import { useCallback, useEffect, useRef, useState } from "react";

/** How long the field waits after the last keystroke before it changes the filters. */
export const SEARCH_DELAY_MS = 300;

export interface DebouncedText {
  /** What the field shows: every keystroke at once, with its spaces. */
  draft: string;
  /** Call it with each new text of the field. */
  change: (text: string) => void;
  /** Commits the draft now, for Enter and for leaving the field. */
  flush: () => void;
}

/**
 * The text of a search field that lives somewhere else, for example in the query string. The field
 * keeps a draft; the owner gets the text 300 ms after the last keystroke, at once when the text is
 * empty, and on `flush`. So a fast typist or a held Backspace makes one change, not one for each
 * key. A change of `committed` that this hook did not write (back, forward, a reset) replaces the
 * draft and drops a commit that is still waiting. So does back or forward, also to an entry with
 * the same text, because a commit would write onto that entry. The timer lives as long as the
 * component that calls the hook, so call it above a sheet that can close while the member types.
 * It does not commit on unmount: a navigation would write the text onto the new address.
 */
export function useDebouncedText(
  committed: string,
  commit: (text: string) => void,
  delay = SEARCH_DELAY_MS,
): DebouncedText {
  const [draft, setDraft] = useState(committed);
  const written = useRef(committed);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const latest = useRef({ draft, commit, committed });
  useEffect(() => {
    latest.current = { draft, commit, committed };
  });

  const stop = useCallback(() => {
    clearTimeout(timer.current);
    timer.current = undefined;
  }, []);

  const write = useCallback(
    (text: string) => {
      stop();
      written.current = text.trim();
      latest.current.commit(text);
    },
    [stop],
  );

  const change = useCallback(
    (text: string) => {
      setDraft(text);
      latest.current.draft = text;
      stop();
      if (text.trim() === "") {
        write(text);
      } else {
        timer.current = setTimeout(() => write(text), delay);
      }
    },
    [delay, stop, write],
  );

  const flush = useCallback(() => {
    if (timer.current !== undefined) {
      write(latest.current.draft);
    }
  }, [write]);

  useEffect(() => {
    if (committed !== written.current) {
      stop();
      written.current = committed;
      setDraft(committed);
    }
  }, [committed, stop]);

  // Back and forward fire `popstate`; the own commit replaces the entry and fires nothing.
  useEffect(() => {
    const onPop = () => {
      if (timer.current !== undefined) {
        stop();
        setDraft(latest.current.committed);
      }
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, [stop]);

  useEffect(() => stop, [stop]);

  return { draft, change, flush };
}
