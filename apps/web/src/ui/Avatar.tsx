import styles from "./Avatar.module.css";

const COLORS = 6;

const GRAPHEMES = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** The graphemes of a word that start with a letter: an emoji, a flag or a bracket is no initial. */
function lettersOf(word: string): string[] {
  return Array.from(GRAPHEMES.segment(word), ({ segment }) => segment).filter((grapheme) =>
    /^\p{L}/u.test(grapheme),
  );
}

/** Upper case, and one grapheme: „ß“ would become „SS“, so the initial is „S“. */
function upper(grapheme: string): string {
  const [first = ""] = Array.from(
    GRAPHEMES.segment(grapheme.toLocaleUpperCase("de-CH")),
    (part) => part.segment,
  );
  return first;
}

/**
 * The initials of a name: the first letters of the first and the last word, or the first two
 * letters of a single word. A word without a letter, such as an emoji, does not count.
 */
export function initialsOf(name: string): string {
  const words = name
    .normalize("NFC")
    .split(/\s+/)
    .map(lettersOf)
    .filter((letters) => letters.length > 0);
  const [first] = words;
  if (first === undefined) {
    return "?";
  }
  const last = words[words.length - 1] ?? first;
  const letters = words.length > 1 ? [first[0], last[0]] : first.slice(0, 2);
  return letters.map((letter) => upper(letter ?? "")).join("");
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
  // An image without a name says nothing to a screen reader.
  const hidden = decorative || name.trim() === "";
  return (
    <span
      className={styles.avatar}
      data-size={size}
      data-color={avatarColor(id ?? name)}
      {...(hidden ? { "aria-hidden": true } : { role: "img", "aria-label": name })}
    >
      <span aria-hidden="true">{initialsOf(name)}</span>
    </span>
  );
}
