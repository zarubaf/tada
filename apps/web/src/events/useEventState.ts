import { type Dispatch, type SetStateAction, useCallback, useEffect, useState } from "react";
import { type Api, type Event, problemMessage } from "../api/client";
import { type FocusTarget, useRetry } from "../ui/focus";

export type EventState =
  | { kind: "loading" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "loaded"; event: Event };

export interface EventLoad {
  state: EventState;
  setState: Dispatch<SetStateAction<EventState>>;
  /** Loads the event again without the loading state. Resolves to true when it loaded. */
  load: () => Promise<boolean>;
  /** True after a press on „Erneut versuchen“: the next failure takes focus. */
  retried: boolean;
  /** For „Erneut versuchen“: shows the loading state, then loads again. Success moves focus. */
  retry: () => void;
}

/**
 * Loads one event. An event that is unknown, or that the member may not see, answers with a
 * problem; the state keeps its message for `EventLoadError`. `target` is the heading of the page,
 * which takes focus when a retry succeeds.
 */
export function useEventState(api: Api, eventId: string, target: FocusTarget): EventLoad {
  const [state, setState] = useState<EventState>({ kind: "loading" });

  const load = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/events/{event_id}", {
        params: { path: { event_id: eventId } },
      });
      setState(
        data
          ? { kind: "loaded", event: data }
          : { kind: "failed", message: problemMessage(error), requestId: error?.request_id },
      );
      return data !== undefined;
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
      return false;
    }
  }, [api, eventId]);

  useEffect(() => {
    void load();
  }, [load]);

  const { retried, retry } = useRetry(target);
  const retryLoad = useCallback(
    () =>
      retry(() => {
        setState({ kind: "loading" });
        return load();
      }),
    [retry, load],
  );

  return { state, setState, load, retried, retry: retryLoad };
}
