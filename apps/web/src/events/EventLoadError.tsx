import { InlineError } from "../ui/InlineError";
import type { EventLoad, EventState } from "./useEventState";

/**
 * The failure of the event that a page belongs to, for example one that is unknown or forbidden,
 * with „Erneut versuchen“. The message is the one of the problem.
 */
export function EventLoadError({
  failure,
  retried,
  onRetry,
}: {
  failure: Extract<EventState, { kind: "failed" }>;
  retried: EventLoad["retried"];
  onRetry: EventLoad["retry"];
}) {
  return (
    <InlineError
      message={failure.message}
      requestId={failure.requestId}
      onRetry={onRetry}
      announce={retried ? "focus" : "alert"}
    />
  );
}
