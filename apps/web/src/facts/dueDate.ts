// The due dates of "My Work". A due date is a civil date (`2030-05-18`) and compares as text.

/**
 * Today as a civil date in the time zone of the browser. `GET /api/v1/me/work` spans many events
 * and gives no time zone per item, so the member's own calendar day decides what is overdue.
 */
export function todayLocal(now: Date = new Date()): string {
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

/** A record is overdue when its due date lies before today. A record without a due date never is. */
export function isOverdue(dueDate: string | null | undefined, today: string): boolean {
  return dueDate != null && dueDate < today;
}
