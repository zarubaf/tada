/** Today as a civil date in the time zone of the event, so that „heute“ is the day of the event. */
export function todayIn(timeZone: string, now = new Date()): string {
  try {
    // `en-CA` writes a date as 2030-05-18.
    return new Intl.DateTimeFormat("en-CA", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).format(now);
  } catch {
    return now.toISOString().slice(0, 10);
  }
}
