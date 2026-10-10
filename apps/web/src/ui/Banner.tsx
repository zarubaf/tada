import type { Icon } from "@tabler/icons-react";
import type { ReactNode, Ref } from "react";
import styles from "./Banner.module.css";

export interface BannerProps {
  icon: Icon;
  /** The message. It names the state in its first sentence. */
  children: ReactNode;
  /** A button that acts on the state, if the member can act. */
  action?: ReactNode;
  /** For the focus that moves to the banner when the state changes. */
  ref?: Ref<HTMLDivElement>;
}

/**
 * A message for a whole page that stays while the state lasts (doc/design/components.md). It is
 * not a live region: the action that causes the state announces its result.
 */
export function Banner({ icon: Icon, children, action, ref }: BannerProps) {
  return (
    <div ref={ref} tabIndex={-1} className={styles.banner}>
      <Icon size={20} stroke={1.5} aria-hidden="true" className={styles.icon} />
      <div className={styles.text}>{children}</div>
      {action && <div className={styles.action}>{action}</div>}
    </div>
  );
}
