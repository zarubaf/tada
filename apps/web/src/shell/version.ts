// The build of the web client and its comparison with the server (ADR 0075, ADR 0078).
import { useEffect, useState } from "react";
import type { Api, SystemVersion } from "../api/client";

/**
 * The opaque ID that `vite.config.ts` writes into this build. The client holds no version and no
 * commit: a client that is not signed in must not learn them (ADR 0078).
 */
export const CLIENT_BUILD_ID: string = __TADA_BUILD_ID__;

/** The text of the footer from the answer of the server, for example `tada 0.3.0 · 1a2b3c4`. */
export function footerText(server: Pick<SystemVersion, "product" | "commit">): string {
  return `tada ${server.product} · ${server.commit}`;
}

/** True if the server delivers another web client than this one: the member must load the page again. */
export function outdated(server: Pick<SystemVersion, "web_build">): boolean {
  return server.web_build !== CLIENT_BUILD_ID;
}

/** A tab that comes back to the front checks again, but at most once in this time. */
const RECHECK_MS = 60_000;

/**
 * The versions of the server, or `undefined` before the first answer. The hook asks the server
 * when the shell mounts and when the tab comes back to the front, because a tab can stay open over
 * an upgrade. A failed request keeps the last answer. Without `signedIn` it asks nothing: tada
 * shows no version to a client that is not signed in.
 */
export function useServerVersion(api: Api, signedIn: boolean): SystemVersion | undefined {
  const [server, setServer] = useState<SystemVersion>();
  useEffect(() => {
    if (!signedIn) {
      return;
    }
    let active = true;
    let checkedAt = Number.NEGATIVE_INFINITY;
    const check = async () => {
      if (Date.now() - checkedAt < RECHECK_MS) {
        return;
      }
      checkedAt = Date.now();
      try {
        const { data } = await api.GET("/api/v1/system/version");
        if (active && data) {
          setServer(data);
        }
      } catch {
        // The next check tries again.
      }
    };
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        void check();
      }
    };
    void check();
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      active = false;
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [api, signedIn]);
  return signedIn ? server : undefined;
}
