// The level of a rating as three bars of growing height: as many bars are full as the level says,
// the others are faint. The count of full bars is the sign, so that it needs no color; the word
// beside it names the level, and the icon is a decoration.
import type { Level } from "../../api/client";
import type { EnumIcon } from "./descriptor";

const BARS = [
  { x: 1, height: 4 },
  { x: 5.5, height: 7 },
  { x: 10, height: 10 },
] as const;
const FULL: Record<Level, number> = { low: 1, medium: 2, high: 3 };
const FLOOR = 12;

function levelIcon(level: Level): EnumIcon {
  return function LevelIcon({ size = 14 }) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 14 14"
        fill="currentColor"
        aria-hidden="true"
        focusable="false"
      >
        {BARS.map((bar, index) => (
          <rect
            key={bar.x}
            x={bar.x}
            y={FLOOR - bar.height}
            width="3"
            height={bar.height}
            rx="0.75"
            opacity={index < FULL[level] ? 1 : 0.3}
          />
        ))}
      </svg>
    );
  };
}

/** The icon of each level. */
export const LEVEL_ICONS: Record<Level, EnumIcon> = {
  low: levelIcon("low"),
  medium: levelIcon("medium"),
  high: levelIcon("high"),
};
