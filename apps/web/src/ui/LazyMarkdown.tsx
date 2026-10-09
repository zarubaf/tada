import {
  Component,
  type ComponentType,
  lazy,
  type ReactNode,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { problemMessage } from "../api/client";
import { InlineError } from "./InlineError";
import type { MarkdownProps } from "./Markdown";
import { Skeleton } from "./Skeleton";

type Loader = () => Promise<{ Markdown: ComponentType<MarkdownProps> }>;

// The Markdown parser is large. It loads with the first text, so that it stays out of the
// initial JavaScript (ADR 0024).
const loadMarkdown: Loader = () => import("./Markdown");

/** Catches the failure of the chunk, for example after a new release removed it. */
class ChunkBoundary extends Component<
  { children: ReactNode; fallback: ReactNode },
  { failed: boolean }
> {
  override state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  override render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

/** Reports that the text is in the page. */
function Shown({ onShown }: { onShown: () => void }) {
  // Once, when the text mounts.
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new callback must not report again.
  useEffect(onShown, []);
  return null;
}

/**
 * `Markdown` in a chunk of its own. While it loads, a skeleton stands in. If the chunk fails, the
 * area shows the failure with „Erneut versuchen“ instead of taking the page down. Use it for each
 * text that a page shows; `ui/Markdown` stays the one renderer (ADR 0058).
 */
export function LazyMarkdown({
  children,
  renderLink,
  onShown,
  load = loadMarkdown,
}: {
  children: string;
  /** Renders a `tada:` link; see `Markdown`. */
  renderLink?: MarkdownProps["renderLink"];
  /** Called when the text is in the page, for example to show an action only after it. */
  onShown?: () => void;
  /** For tests: the loader of the chunk. */
  load?: Loader;
}) {
  const [attempt, setAttempt] = useState(0);
  // A rejected `lazy` stays rejected, so each attempt makes a new one.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` makes a new component.
  const Lazy = useMemo(
    () => lazy(() => load().then(({ Markdown }) => ({ default: Markdown }))),
    [load, attempt],
  );
  const area = useRef<HTMLDivElement>(null);
  // True after a press on „Erneut versuchen“: the next failure takes focus, a success moves it to the text.
  const [retried, setRetried] = useState(false);
  // The boundary stays failed until it mounts again: a new key does that.
  const [failures, setFailures] = useState(0);

  const shown = () => {
    if (retried) {
      setRetried(false);
      area.current?.focus();
    }
    onShown?.();
  };
  const fallback = (
    <InlineError
      message={problemMessage(undefined)}
      onRetry={() => {
        setRetried(true);
        setAttempt((n) => n + 1);
        setFailures((n) => n + 1);
      }}
      announce={retried ? "focus" : "alert"}
    />
  );
  return (
    <div ref={area} tabIndex={-1}>
      <ChunkBoundary key={failures} fallback={fallback}>
        <Suspense fallback={<Skeleton />}>
          <Lazy renderLink={renderLink}>{children}</Lazy>
          <Shown onShown={shown} />
        </Suspense>
      </ChunkBoundary>
    </div>
  );
}
