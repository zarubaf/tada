import { lazy, Suspense } from "react";
import type { Api } from "../api/client";
import { Button } from "../ui/Button";
import styles from "./VersionFooter.module.css";
import { CLIENT } from "./version";

// The list of the components loads on demand: a member opens it seldom (ADR 0024).
const ComponentList = lazy(() =>
  import("./ComponentList").then((module) => ({ default: module.ComponentList })),
);

/**
 * The build of this client in the footer of the sidebar, for example `tada 0.3.0 · 1a2b3c4`
 * (ADR 0075). A press opens the list of the components. Only the signed-in shell shows it.
 */
export function VersionButton({ onPress }: { onPress: () => void }) {
  return (
    <Button variant="quiet" onPress={onPress} aria-haspopup="dialog">
      <span className={styles.text}>
        tada {CLIENT.version} · <span className={styles.commit}>{CLIENT.commit}</span>
      </span>
    </Button>
  );
}

/** The list of the components while `isOpen`; its code loads with the first opening. */
export function VersionSheet({
  api,
  isOpen,
  onClose,
}: {
  api: Api;
  isOpen: boolean;
  onClose: () => void;
}) {
  return isOpen ? (
    <Suspense fallback={null}>
      <ComponentList api={api} onClose={onClose} />
    </Suspense>
  ) : null;
}
