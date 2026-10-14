import styles from "./Avatar.module.css";

const COLORS = 6;

/** The initials of a name: first and last word, or the first two letters of a single word. */
export function initialsOf(name: string): string {
  const words = name.split(/\s+/).filter(Boolean);
  const [first] = words;
  if (first === undefined) {
    return "?";
  }
  const letters =
    words.length > 1
      ? [first, words[words.length - 1] as string].map((word) => Array.from(word)[0])
      : Array.from(first).slice(0, 2);
  return letters.join("").toLocaleUpperCase("de-CH");
}

/** The number 1 to 6 of the palette color (`--color-avatar-N`) of a key. The same key, the same color. */
export function avatarColor(key: string): number {
  let hash = 0;
  for (const unit of key) {
    hash = (Math.imul(hash, 31) + (unit.codePointAt(0) ?? 0)) >>> 0;
  }
  return (hash % COLORS) + 1;
}

export interface AvatarProps {
  /** The name of the person. It gives the initials and the accessible name. */
  name: string;
  /** The ID of the person. The color follows it, so a renamed person keeps the color. */
  id?: string;
  size?: 20 | 24;
  /** The name stands next to the avatar, so a screen reader skips the avatar. */
  decorative?: boolean;
}

/** The initials of a person on a color of the fixed palette (doc/design/components.md). */
export function Avatar({ name, id, size = 20, decorative = false }: AvatarProps) {
  return (
    <span
      className={styles.avatar}
      data-size={size}
      data-color={avatarColor(id ?? name)}
      {...(decorative ? {} : { role: "img", "aria-label": name })}
    >
      <span aria-hidden="true">{initialsOf(name)}</span>
    </span>
  );
}
