import {
  IconAlertTriangle,
  IconDiamond,
  IconGavel,
  IconHeartHandshake,
  IconSquareCheck,
} from "@tabler/icons-react";
import type { RecordKind } from "../api/client";
import { t } from "../i18n";
import type { IconSize } from "./StatusIcon";
import styles from "./TypeIcon.module.css";

const ICONS = {
  action: IconSquareCheck,
  commitment: IconHeartHandshake,
  milestone: IconDiamond,
  decision: IconGavel,
  risk: IconAlertTriangle,
} satisfies Record<RecordKind, unknown>;

export interface TypeIconProps {
  kind: RecordKind;
  size?: IconSize;
  /** The name of the kind stands next to the icon, so a screen reader skips the icon. */
  decorative?: boolean;
}

/** The icon of a record kind: Aufgabe, Zusage, Meilenstein, Entscheid or Risiko. */
export function TypeIcon({ kind, size = 16, decorative = false }: TypeIconProps) {
  const Icon = ICONS[kind];
  return (
    <Icon
      className={styles.icon}
      size={size}
      stroke={1.5}
      {...(decorative
        ? { "aria-hidden": true }
        : { role: "img", "aria-label": t(`record-kind-${kind}`) })}
    />
  );
}
