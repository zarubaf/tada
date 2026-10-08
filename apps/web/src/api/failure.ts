// One rule that maps a failed request to what the page shows, including the wait of a 429.
import { useEffect, useState } from "react";
import { t } from "../i18n";
import { type Problem, problemMessage } from "./client";

export interface Failure {
  /** A new value for each failure, so that a page can mount its alert again and move focus to it. */
  id: number;
  message: string;
  requestId: string | undefined;
  /** The wait that the server asks for (`Retry-After`), in seconds. */
  retryAfter: number | undefined;
  /** The link or the invitation cannot work: a retry makes no sense. */
  final: boolean;
}

let sequence = 0;

// These 4xx codes do not say that the credential is bad: the member can try again later.
const RETRYABLE_4XX = [403, 408, 429];

function retryAfterSeconds(response: Response | undefined): number | undefined {
  const seconds = Number(response?.headers.get("Retry-After"));
  return Number.isInteger(seconds) && seconds > 0 ? seconds : undefined;
}

/** The message of a 429 names the wait if the server sent one. */
function rateLimitedMessage(seconds: number): string {
  return seconds >= 120
    ? t("problem-rate-limited-minutes", { minutes: Math.ceil(seconds / 60) })
    : t("problem-rate-limited-seconds", { seconds });
}

/** The failure of a link or an invitation that cannot work, for example without a token. */
export function invalidFailure(message: string): Failure {
  return { id: ++sequence, message, requestId: undefined, retryAfter: undefined, final: true };
}

/**
 * The failure of a response. `invalidMessage` is for a page that uses a token: any other 4xx means
 * that the token does not work. Without it, each problem shows the message of its code.
 */
export function failureOf(
  result: { error?: Problem | undefined; response?: Response | undefined },
  invalidMessage?: string,
): Failure {
  const { error, response } = result;
  if (error && invalidMessage && error.status < 500 && !RETRYABLE_4XX.includes(error.status)) {
    return invalidFailure(invalidMessage);
  }
  const id = ++sequence;
  const retryAfter = error?.status === 429 ? retryAfterSeconds(response) : undefined;
  return {
    id,
    message: retryAfter ? rateLimitedMessage(retryAfter) : problemMessage(error),
    requestId: error?.request_id,
    retryAfter,
    final: false,
  };
}

/** True while the server asks the member to wait. */
export function useWaiting(failure: Failure | undefined): boolean {
  const [waiting, setWaiting] = useState(false);
  useEffect(() => {
    if (!failure?.retryAfter) {
      setWaiting(false);
      return;
    }
    setWaiting(true);
    const timer = setTimeout(() => setWaiting(false), failure.retryAfter * 1000);
    return () => clearTimeout(timer);
  }, [failure]);
  return waiting;
}
