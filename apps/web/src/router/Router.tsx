// A small router of our own (OP5). It maps the path of the address to a page. It has no loaders
// and no nesting rules: a page that needs data loads it itself.
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

  useEffect(() => {
    const sync = () => setPathname(window.location.pathname);
    window.addEventListener("popstate", sync);
    return () => window.removeEventListener("popstate", sync);
  }, []);

  const navigate = useCallback((to: string, options?: { replace?: boolean }) => {
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
      params[segment.slice(1)] = decodeURIComponent(value);
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
  children: ReactNode;
}

/** A link that changes the path without a page load. The current page gets `aria-current`. */
export function Link({ to, className, children }: LinkProps) {
  const { pathname, navigate } = useLocation();
  const current = pathname === to || pathname.startsWith(`${to}/`);
  const onClick = (event: MouseEvent<HTMLAnchorElement>) => {
    const plain = event.button === 0 && !(event.metaKey || event.ctrlKey || event.shiftKey);
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
