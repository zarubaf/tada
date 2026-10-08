// The session of the member: who is signed in, and in which organization. It is the one place
// that reacts to `unauthenticated` and `organization-required` (ADR 0037, ADR 0056).
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  type Api,
  type Membership,
  problemMessage,
  type SessionInfo,
  watchSessionProblems,
} from "../api/client";
import { t } from "../i18n";
import { Redirect, useNavigate, usePathname } from "../router/Router";
import { InlineError } from "../ui/InlineError";
import { Skeleton } from "../ui/Skeleton";
import { CHOOSE_ORGANIZATION_PATH, isPublicPath, SIGN_IN_PATH } from "./paths";

export interface Session {
  user: { id: string; displayName: string };
  /** Absent until the member chooses an organization. */
  organization: Membership | undefined;
  memberships: Membership[];
  /** Loads the session again, for example after a change of the organization. */
  refresh: () => Promise<void>;
  /** Ends the session. Resolves to the message of a failure, or nothing. */
  signOut: () => Promise<string | undefined>;
}

type State =
  | { kind: "loading" }
  | { kind: "signed-out" }
  | { kind: "failed"; message: string; requestId: string | undefined }
  | { kind: "ready"; info: SessionInfo };

const SessionContext = createContext<Session | null>(null);
const RefreshContext = createContext<(() => Promise<void>) | null>(null);

/**
 * Loads the session again. A public page calls it after it created a session, for example after
 * the click on a magic link. It works without a session.
 */
export function useRefreshSession(): () => Promise<void> {
  const refresh = useContext(RefreshContext);
  if (!refresh) {
    throw new Error("useRefreshSession needs a <SessionProvider>");
  }
  return refresh;
}

/** The session, or nothing on a public page where nobody has signed in. */
export function useOptionalSession(): Session | null {
  return useContext(SessionContext);
}

export function useSession(): Session {
  const session = useOptionalSession();
  if (!session) {
    throw new Error("useSession needs a signed-in member");
  }
  return session;
}

export function SessionProvider({ api, children }: { api: Api; children: ReactNode }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const navigate = useNavigate();
  const pathname = usePathname();
  const isPublic = isPublicPath(pathname);
  const onPublicPath = useRef(isPublic);
  onPublicPath.current = isPublic;

  const refresh = useCallback(async () => {
    try {
      const { data, error } = await api.GET("/api/v1/session");
      if (data) {
        setState({ kind: "ready", info: data });
      } else if (error?.code === "unauthenticated") {
        setState({ kind: "signed-out" });
      } else {
        setState({ kind: "failed", message: problemMessage(error), requestId: error?.request_id });
      }
    } catch {
      setState({ kind: "failed", message: problemMessage(undefined), requestId: undefined });
    }
  }, [api]);

  const signOut = useCallback(async () => {
    try {
      const { error } = await api.POST("/api/v1/sign-out");
      if (error) {
        return problemMessage(error);
      }
    } catch {
      return problemMessage(undefined);
    }
    setState({ kind: "signed-out" });
    navigate(SIGN_IN_PATH, { replace: true });
    return undefined;
  }, [api, navigate]);

  useEffect(() => {
    const watcher = watchSessionProblems((code) => {
      if (code === "unauthenticated") {
        setState({ kind: "signed-out" });
        if (!onPublicPath.current) {
          navigate(SIGN_IN_PATH, { replace: true });
        }
      } else {
        navigate(CHOOSE_ORGANIZATION_PATH, { replace: true });
      }
    });
    api.use(watcher);
    void refresh();
    return () => api.eject(watcher);
  }, [api, navigate, refresh]);

  const session = useMemo<Session | null>(
    () =>
      state.kind === "ready"
        ? {
            user: { id: state.info.user_id, displayName: state.info.display_name },
            organization: state.info.organization ?? undefined,
            memberships: state.info.memberships,
            refresh,
            signOut,
          }
        : null,
    [state, refresh, signOut],
  );

  return <RefreshContext value={refresh}>{view()}</RefreshContext>;

  // A public page works without a session. A member who has one leaves the sign-in page.
  function view() {
    if (isPublic && !session) {
      // The same element as the last return: a page that created the session (a magic link) must
      // not mount again, or it would lose the token that it read from the address.
      return <SessionContext value={null}>{children}</SessionContext>;
    }
    if (session && pathname === SIGN_IN_PATH) {
      return <Redirect to="/events" />;
    }
    if (state.kind === "failed") {
      return (
        <InlineError
          message={state.message}
          requestId={state.requestId}
          onRetry={() => {
            setState({ kind: "loading" });
            void refresh();
          }}
        />
      );
    }
    if (state.kind === "signed-out") {
      return <Redirect to={SIGN_IN_PATH} />;
    }
    if (!session) {
      return (
        <div role="status" aria-label={t("session-loading")}>
          <Skeleton />
        </div>
      );
    }
    if (!session.organization && !isPublic && pathname !== CHOOSE_ORGANIZATION_PATH) {
      return <Redirect to={CHOOSE_ORGANIZATION_PATH} />;
    }
    return <SessionContext value={session}>{children}</SessionContext>;
  }
}
