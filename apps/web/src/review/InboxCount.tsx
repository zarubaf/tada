import { t } from "../i18n";
import styles from "./InboxCount.module.css";
import { useOptionalInbox } from "./InboxProvider";

/**
 * The number of changesets that wait for a review, for the navigation item „Eingang“. It shows
 * nothing while the list loads, when it failed, and when nothing waits. A screen reader hears
 * „3 offen“; the digit alone would not say what it counts.
 */
export function InboxCount() {
  const state = useOptionalInbox()?.state;
  if (state?.kind !== "loaded" || state.items.length === 0) {
    return null;
  }
  const count = state.items.length;
  return (
    <>
      <span className={styles.count} aria-hidden="true">
        {state.more ? `${count}+` : count}
      </span>
      <span className={styles.hidden}>{t("nav-inbox-count", { count })}</span>
    </>
  );
}
