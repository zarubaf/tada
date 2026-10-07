/**
 * The timeout of a test that types into a form or opens a React Aria popover or dialog.
 * Such a test takes 2 to 4 s in jsdom and more than the default 5 s on a loaded machine.
 */
export const SLOW = 15_000;
