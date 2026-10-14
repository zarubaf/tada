/**
 * Three bars on a time axis, for the timeline. The Tabler set has no gantt icon (its timeline icon
 * is a line chart), so this one is drawn in the same way: 24 px grid, round caps, a stroke.
 */
export function IconGantt({ size = 14, stroke = 1.5 }: { size?: number; stroke?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={stroke}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M3 3v16a2 2 0 0 0 2 2h16" />
      <path d="M7 7h7" />
      <path d="M11 12h8" />
      <path d="M8 17h5" />
    </svg>
  );
}
