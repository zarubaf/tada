import { lazy, Suspense } from "react";
import type { Api, SystemVersion } from "../api/client";
import { t } from "../i18n";
import { Button } from "../ui/Button";
import styles from "./VersionFooter.module.css";

// The list of the components loads on demand: a member opens it seldom (ADR 0024).
const ComponentList = lazy(() =>
  import("./ComponentList").then((module) => ({ default: module.ComponentList })),
);

/**
 * The build of the server in the footer of the sidebar, for example `tada 0.3.0 · 1a2b3c4`
 * (ADR 0075). The text comes from the API, so only the signed-in shell has it (ADR 0078); until the
 * first answer the button says „Versionen“. A press opens the list of the components.
 */
export function VersionButton({
  server,
  onPress,
}: {
  server: SystemVersion | undefined;
  onPress: () => void;
}) {
  return (
    <Button variant="quiet" onPress={onPress} aria-haspopup="dialog">
      <span className={styles.text}>
        {server ? (
          <>
            tada {server.product} · <span className={styles.commit}>{server.commit}</span>
          </>
        ) : (
          t("versions-title")
        )}
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
