// The IANA time zones that the browser knows, for the choice of a time zone.

/** `America/New_York` as `America/New York`: the ID stays recognizable and reads as words. */
export function timeZoneLabel(zone: string): string {
  return zone.replaceAll("_", " ");
}

/**
 * The time zones of the browser in alphabetic order, with `current` among them. The server can know
 * a zone that the browser does not list, for example `UTC`, and the member must still see it.
 */
export function timeZonesWith(current: string | undefined): string[] {
  const known = Intl.supportedValuesOf("timeZone");
  return current === undefined || known.includes(current) ? known : [...known, current].sort();
}
