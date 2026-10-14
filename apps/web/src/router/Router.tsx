// A small router of our own, without a new dependency. It maps the path of the address to a page.
// The query string holds the state of a view (tabs, filters); the hash names the open record.
// Only a change of the path is a new page: a change of the query or the hash keeps focus and scroll.
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
  useMemo,
  useRef,
  useState,
} from "react";

type Params = Record<string, string>;

export interface NavigateOptions {
  /** Replaces the current history entry instead of a new one. */
  replace?: boolean;
  /**
   * Moves focus to the `h1` of the new page (doc/design/accessibility.md). Only a redirect that no
   * member caused turns it off; after a member action the pressed control has left the page.
   */
  moveFocus?: boolean;
}

/** The parts of the address that the router follows. */
interface Address {
  pathname: string;
  /** The query string as the address has it, with the leading `?` or empty. */
  search: string;
  /** The fragment of the address without `#`, for example the readable ID of a row. */
  hash: string;
}

/** The read side of the query string. It never changes: a new location gives a new accessor. */
export interface SearchParams {
  get: (name: string) => string | null;
  getAll: (name: string) => string[];
  has: (name: string) => boolean;
}

/**
 * Changes by name: a string sets the parameter, an array sets it once for each value, `null` or
 * `undefined` removes it. The caller drops a parameter with its default value; the router does not
 * know the defaults.
 */
export type SearchUpdate = Record<string, string | readonly string[] | null | undefined>;

interface Location extends Address {
  navigate: (to: string, options?: NavigateOptions) => void;
}

const LocationContext = createContext<Location | null>(null);
const ParamsContext = createContext<Params>({});
// Settles the pending focus request once the page is in. Outside a router it does nothing.
const SettleFocusContext = createContext<() => void>(() => {});

/**
 * A lazy page is not in the DOM when the route changes, so the router cannot find its `h1` then.
 * `useSettleFocus()` returns the function to call when the page has mounted: it moves focus to the
 * `h1` if a navigation still waits for it.
 */
export function useSettleFocus(): () => void {
  return useContext(SettleFocusContext);
}

const PENDING = "[data-route-pending]";

function useLocation(): Location {
  const location = useContext(LocationContext);
  if (!location) {
    throw new Error("the router hooks need a <Router>");
  }
  return location;
}

function readHash(): string {
  try {
    return decodeURIComponent(window.location.hash.slice(1));
  } catch {
    return "";
  }
}

function readAddress(): Address {
  const { pathname, search } = window.location;
  return { pathname, search, hash: readHash() };
}

function sameAddress(a: Address, b: Address): boolean {
  return a.pathname === b.pathname && a.search === b.search && a.hash === b.hash;
}

/** Keeps the address in state and changes it with `history.pushState`. */
export function Router({ children }: { children: ReactNode }) {
  const [address, setAddress] = useState(readAddress);
  const { pathname, search, hash } = address;
  // Focus moves to the new page after a navigation, not after a redirect or the first render.
  const moveFocus = useRef(false);

  // Takes the address of the window. A new page asks for focus; a new query or hash does not.
  const shown = useRef(address);
  const sync = useCallback((focus: boolean) => {
    const next = readAddress();
    const previous = shown.current;
    if (sameAddress(previous, next)) {
      return;
    }
    shown.current = next;
    moveFocus.current = focus && previous.pathname !== next.pathname;
    setAddress(next);
  }, []);

  useEffect(() => {
    const onChange = () => sync(true);
    window.addEventListener("popstate", onChange);
    window.addEventListener("hashchange", onChange);
    return () => {
      window.removeEventListener("popstate", onChange);
      window.removeEventListener("hashchange", onChange);
    };
  }, [sync]);

  const settleFocus = useCallback(() => {
    // A fallback in the page is no target: the request waits until the page mounts.
    if (!moveFocus.current || document.querySelector(PENDING)) {
      return;
    }
    moveFocus.current = false;
    const target = document.querySelector<HTMLElement>("h1") ?? document.querySelector("main");
    target?.setAttribute("tabindex", "-1");
    target?.focus();
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: the path change is the trigger
  useEffect(settleFocus, [pathname]);

  const navigate = useCallback(
    (to: string, options?: NavigateOptions) => {
      const { pathname: path, search: query, hash: fragment } = window.location;
      if (to === path + query + fragment) {
        return;
      }
      if (options?.replace) {
        window.history.replaceState(null, "", to);
      } else {
        window.history.pushState(null, "", to);
      }
      sync(options?.moveFocus ?? true);
    },
    [sync],
  );

  return (
    <LocationContext value={{ pathname, search, hash, navigate }}>
      <SettleFocusContext value={settleFocus}>{children}</SettleFocusContext>
    </LocationContext>
  );
}

export function usePathname(): string {
  return useLocation().pathname;
}

/** The fragment of the address without `#`; a link to a row of a register names the row in it. */
export function useHash(): string {
  return useLocation().hash;
}

/**
 * The parameters of the query string. The accessor stays the same while the query does, so it can
 * be a dependency of an effect or a memo. A malformed escape reads as the raw text, never throws.
 */
export function useSearch(): SearchParams {
  const { search } = useLocation();
  return useMemo(() => new URLSearchParams(search), [search]);
}

/**
 * Returns the function that changes parameters and keeps the path and the hash. The caller picks
 * `replace` (a filter) or a new entry (a view tab). Other parameters stay as they are.
 */
export function useSetSearch(): (update: SearchUpdate, options?: NavigateOptions) => void {
  const { navigate } = useLocation();
  return useCallback(
    (update, options) => {
      const params = new URLSearchParams(window.location.search);
      for (const [name, value] of Object.entries(update)) {
        params.delete(name);
        for (const item of typeof value === "string" ? [value] : (value ?? [])) {
          params.append(name, item);
        }
      }
      const query = params.toString();
      const { pathname, hash } = window.location;
      navigate(`${pathname}${query ? `?${query}` : ""}${hash}`, {
        moveFocus: false,
        ...options,
      });
    },
    [navigate],
  );
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

/** Replaces the current history entry with `to`. Focus stays: no member caused the redirect. */
export function Redirect({ to }: { to: string }) {
  const navigate = useNavigate();
  useEffect(() => navigate(to, { replace: true, moveFocus: false }), [navigate, to]);
  return null;
}

export interface LinkProps {
  /** A path with an optional `?query` and `#hash`. */
  to: string;
  className?: string;
  /** The link is current only for its own path, not for the paths below it. */
  exact?: boolean;
  /**
   * The link is also current for every path below this prefix, for example a whole settings area,
   * or the pages of the documents of one event, whose paths do not start with the path of the link.
   */
  within?: string;
  /**
   * The link is also current on each of these paths and the paths below them, for example a link to
   * the first page of a group of pages with a navigation of its own.
   */
  alsoCurrentOn?: readonly string[];
  /** For a link whose words do not name its target alone, for example in a table. */
  "aria-label"?: string;
  /** `-1` takes the link out of the tab order, where another link of the page does the same. */
  tabIndex?: number;
  children: ReactNode;
}

/** A link that changes the path without a page load. The current page gets `aria-current`. */
export function Link({
  to,
  className,
  exact,
  within,
  alsoCurrentOn = [],
  "aria-label": ariaLabel,
  tabIndex,
  children,
}: LinkProps) {
  const { pathname, navigate } = useLocation();
  // A query or a hash in `to` does not count: the link is current for its path.
  const target = to.split(/[?#]/, 1)[0] ?? to;
  const current =
    pathname === target ||
    (!exact && pathname.startsWith(`${target}/`)) ||
    (within !== undefined && pathname.startsWith(`${within}/`)) ||
    alsoCurrentOn.some((path) => pathname === path || pathname.startsWith(`${path}/`));
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
      aria-label={ariaLabel}
      tabIndex={tabIndex}
      aria-current={current ? "page" : undefined}
      onClick={onClick}
    >
      {children}
    </a>
  );
}
