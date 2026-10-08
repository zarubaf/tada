// A small router of our own, without a new dependency. It maps the path of the address to a page.
// It has no loaders and no nesting rules: a page that needs data loads it itself.
// Replace it with a library when the pages need nested layouts or data loading in the router.
import {
  Children,
  createContext,
  isValidElement,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

type Params = Record<string, string>;

interface Location {
  pathname: string;
  navigate: (to: string, options?: { replace?: boolean }) => void;
}

const LocationContext = createContext<Location | null>(null);
const ParamsContext = createContext<Params>({});

function useLocation(): Location {
  const location = useContext(LocationContext);
  if (!location) {
    throw new Error("the router hooks need a <Router>");
  }
  return location;
}

/** Keeps the path of the address in state and changes it with `history.pushState`. */
export function Router({ children }: { children: ReactNode }) {
  const [pathname, setPathname] = useState(() => window.location.pathname);
  // Focus moves to the new page after a member navigates, not after a redirect or the first render.
  const moveFocus = useRef(false);

  useEffect(() => {
    const sync = () => {
      moveFocus.current = true;
      setPathname(window.location.pathname);
    };
    window.addEventListener("popstate", sync);
    return () => window.removeEventListener("popstate", sync);
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: the path change is the trigger
  useEffect(() => {
    if (!moveFocus.current) {
      return;
    }
    moveFocus.current = false;
    const target = document.querySelector<HTMLElement>("h1") ?? document.querySelector("main");
    target?.setAttribute("tabindex", "-1");
    target?.focus();
  }, [pathname]);

  const navigate = useCallback((to: string, options?: { replace?: boolean }) => {
    if (to === window.location.pathname) {
      return;
    }
    moveFocus.current = !options?.replace;
    if (options?.replace) {
      window.history.replaceState(null, "", to);
    } else {
      window.history.pushState(null, "", to);
    }
    setPathname(window.location.pathname);
  }, []);

  return <LocationContext value={{ pathname, navigate }}>{children}</LocationContext>;
}

export function usePathname(): string {
  return useLocation().pathname;
}

export function useNavigate(): Location["navigate"] {
  return useLocation().navigate;
}

export function useParams(): Params {
  return useContext(ParamsContext);
}

/** `:name` segments match one segment. The path `*` matches each path. */
function match(pattern: string, pathname: string): Params | null {
  if (pattern === "*") {
    return {};
  }
  const wanted = pattern.split("/");
  const actual = pathname.split("/");
  if (wanted.length !== actual.length) {
    return null;
  }
  const params: Params = {};
  for (const [index, segment] of wanted.entries()) {
    const value = actual[index] ?? "";
    if (segment.startsWith(":")) {
      try {
        params[segment.slice(1)] = decodeURIComponent(value);
      } catch {
        return null; // a malformed escape never matches
      }
    } else if (segment !== value) {
      return null;
    }
  }
  return params;
}

export interface RouteProps {
  path: string;
  children: ReactNode;
}

/** A page for a path. It does nothing by itself: `Routes` reads its props. */
export function Route({ children }: RouteProps) {
  return children;
}

/** Renders the children of the first `Route` that matches the path. */
export function Routes({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  for (const child of Children.toArray(children)) {
    if (!isValidElement(child)) {
      continue;
    }
    const route = child as ReactElement<RouteProps>;
    const params = match(route.props.path, pathname);
    if (params) {
      return <ParamsContext value={params}>{route.props.children}</ParamsContext>;
    }
  }
  return null;
}

/** Replaces the current history entry with `to`. */
export function Redirect({ to }: { to: string }) {
  const navigate = useNavigate();
  useEffect(() => navigate(to, { replace: true }), [navigate, to]);
  return null;
}

export interface LinkProps {
  to: string;
  className?: string;
  /** The link is current only for its own path, not for the paths below it. */
  exact?: boolean;
  /** The link is current for every path below this prefix, for example a whole settings area. */
  within?: string;
  children: ReactNode;
}

/** A link that changes the path without a page load. The current page gets `aria-current`. */
export function Link({ to, className, exact, within, children }: LinkProps) {
  const { pathname, navigate } = useLocation();
  const current = within
    ? pathname.startsWith(`${within}/`)
    : pathname === to || (!exact && pathname.startsWith(`${to}/`));
  const onClick = (event: MouseEvent<HTMLAnchorElement>) => {
    const plain =
      event.button === 0 &&
      !(event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) &&
      !event.defaultPrevented &&
      !event.currentTarget.target;
    if (plain) {
      event.preventDefault();
      navigate(to);
    }
  };
  return (
    <a
      href={to}
      className={className}
      aria-current={current ? "page" : undefined}
      onClick={onClick}
    >
      {children}
    </a>
  );
}
