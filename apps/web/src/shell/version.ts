// The build identity of the web client and its comparison with the server (ADR 0075).
import { useEffect, useState } from "react";
import type { Api, SystemVersion } from "../api/client";

export interface BuildIdentity {
  version: string;
  commit: string;
}

/** The identity that `vite.config.ts` writes into this build. */
export const CLIENT: BuildIdentity = { version: __TADA_VERSION__, commit: __TADA_COMMIT__ };

/** The text of the footer, for example `tada 0.3.0 · 1a2b3c4`. */
export function footerText(identity: BuildIdentity = CLIENT): string {
  return `tada ${identity.version} · ${identity.commit}`;
}

/**
 * True if `other` runs another build than `own`. A build without Git gives the commit `unknown`;
 * then only the versions count.
 */
export function differs(own: BuildIdentity, other: BuildIdentity): boolean {
  const commitsKnown = own.commit !== "unknown" && other.commit !== "unknown";
  return own.version !== other.version || (commitsKnown && own.commit !== other.commit);
}

/** The server as a build identity. */
export function serverIdentity(server: SystemVersion): BuildIdentity {
  return { version: server.product, commit: server.commit };
}

/** A tab that comes back to the front checks again, but at most once in this time. */
const RECHECK_MS = 60_000;

/**
 * True when the server runs another build than this client: the member must load the page again.
 * The hook asks the server when the shell mounts and when the tab comes back to the front, because
 * a tab can stay open over an upgrade. A failed request changes nothing. Without `signedIn` it
 * asks nothing: tada shows no version to a client that is not signed in.
 */
export function useOutdatedClient(api: Api, signedIn: boolean): boolean {
  const [outdated, setOutdated] = useState(false);
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
          setOutdated(differs(CLIENT, serverIdentity(data)));
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
  return signedIn && outdated;
}
