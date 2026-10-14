import {
  type Icon,
  IconAlertOctagon,
  IconCircleCheckFilled,
  IconCircleDashed,
  IconCircleHalf2,
  IconCircleMinus,
  IconCircleXFilled,
  IconHourglass,
  IconLock,
  IconReplace,
  IconShieldCheck,
} from "@tabler/icons-react";
import styles from "./StatusIcon.module.css";

export type StatusTone = "neutral" | "progress" | "warning" | "success" | "danger" | "muted";

/** The icon sizes of ADR 0021: 12 in chips, 14 in dense rows, 16 and 20 elsewhere. */
export type IconSize = 12 | 14 | 16 | 20;

/**
 * One drawn icon for each shape of a status. The first five are the icons of ADR 0082: open
 * (dashed circle), in progress (half-filled circle), blocked (circle with a bar), done (circle with
 * a check) and canceled (circle with a cross). The others belong to one kind.
 */
export type StatusGlyph =
  | "open"
  | "progress"
  | "blocked"
  | "done"
  | "canceled"
  | "conditional"
  | "firm"
  | "superseded"
  | "mitigated"
  | "occurred";

const GLYPHS = {
  open: IconCircleDashed,
  progress: IconCircleHalf2,
  blocked: IconCircleMinus,
  done: IconCircleCheckFilled,
  canceled: IconCircleXFilled,
  conditional: IconHourglass,
  firm: IconLock,
  superseded: IconReplace,
  mitigated: IconShieldCheck,
  occurred: IconAlertOctagon,
} satisfies Record<StatusGlyph, Icon>;

export interface ToneIconProps {
  icon: Icon;
  tone: StatusTone;
  size?: IconSize;
  /** The accessible name. Without it, the icon is decoration next to its text. */
  label?: string;
}

/** An icon in the color of a status tone. The tone only supports the text, never replaces it. */
export function ToneIcon({ icon: Icon, tone, size = 16, label }: ToneIconProps) {
  return (
    <Icon
      className={styles.icon}
      data-tone={tone}
      size={size}
      stroke={1.5}
      {...(label === undefined ? { "aria-hidden": true } : { role: "img", "aria-label": label })}
    />
  );
}

export interface StatusIconProps extends Omit<ToneIconProps, "icon"> {
  glyph: StatusGlyph;
}

/** The icon of a status. It has an accessible name, or it stands next to the label of the status. */
export function StatusIcon({ glyph, ...props }: StatusIconProps) {
  return <ToneIcon icon={GLYPHS[glyph]} {...props} />;
}

/** The icon of a glyph, for a component that places the icon itself. */
export function glyphIcon(glyph: StatusGlyph): Icon {
  return GLYPHS[glyph];
}
