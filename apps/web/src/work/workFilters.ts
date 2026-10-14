// The filters of the „Arbeit“ page as pure functions (ADR 0082): which work items show, how they
// sort, how the board groups them, what the counts say and how the filters live in the query
// string. The page keeps the view in the query string too; these functions leave its key alone.
import type { RecordKind } from "../api/client";
import { isOverdue } from "../facts/dueDate";
import { BOARD_COLUMNS, type BoardColumn, RECORD_KINDS } from "./recordStatus";
import type { WorkItem } from "./workItem";

export const DUE_WINDOWS = ["overdue", "week", "none"] as const;
export type DueWindow = (typeof DUE_WINDOWS)[number];

export interface WorkFilters {
  /** Empty means every kind. The same holds for the other lists. */
  kinds: RecordKind[];
  /** User IDs, or `mine` for the signed-in member. */
  owners: string[];
  workstreams: string[];
  due: DueWindow | undefined;
  /** Searches the title and the readable ID. */
  text: string;
}

export const NO_FILTERS: WorkFilters = {
  kinds: [],
  owners: [],
  workstreams: [],
  due: undefined,
  text: "",
};

export interface FilterContext {
  /** The ID of the signed-in member. */
  me: string;
  /** Today as a civil date in the browser's time zone (`todayLocal`). */
  today: string;
}

/**
 * The due date of an item. A decision has the day it was made, not a due date, so it has none; a
 * risk has no date at all.
 */
function dueOf(item: WorkItem): string | undefined {
  return item.kind === "decision" ? undefined : item.date;
}

const CLOSED: readonly BoardColumn[] = ["done", "canceled"];

/** The Sunday of the week of a civil date (the week runs from Monday to Sunday). */
function endOfWeek(day: string): string {
  const date = new Date(`${day}T00:00:00Z`);
  const sinceMonday = (date.getUTCDay() + 6) % 7;
  date.setUTCDate(date.getUTCDate() + 6 - sinceMonday);
  return date.toISOString().slice(0, 10);
}

function inDueWindow(item: WorkItem, window: DueWindow, today: string): boolean {
  const due = dueOf(item);
  if (window === "none") {
    return due === undefined;
  }
  if (due === undefined || CLOSED.includes(item.column)) {
    return false;
  }
  return window === "overdue" ? isOverdue(due, today) : due >= today && due <= endOfWeek(today);
}

/** Lower case without accents, so that a search for "cafe" finds "Café". */
function plain(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Mark}/gu, "")
    .toLowerCase();
}

/** Letters and digits only, so that "act12" and "ACT 012" find ACT-012. */
function alphanumeric(text: string): string {
  return text.replace(/[^a-z0-9]/g, "");
}

type Skip = "kinds" | undefined;

function matches(item: WorkItem, f: WorkFilters, { me, today }: FilterContext, skip: Skip) {
  if (skip !== "kinds" && f.kinds.length > 0 && !f.kinds.includes(item.kind)) {
    return false;
  }
  if (f.owners.length > 0) {
    const owners = f.owners.map((owner) => (owner === "mine" ? me : owner));
    if (!owners.includes(item.owner.id)) {
      return false;
    }
  }
  if (
    f.workstreams.length > 0 &&
    (item.workstreamId === undefined || !f.workstreams.includes(item.workstreamId))
  ) {
    return false;
  }
  if (f.due && !inDueWindow(item, f.due, today)) {
    return false;
  }
  const text = plain(f.text.trim());
  if (text === "") {
    return true;
  }
  const id = alphanumeric(text);
  return (
    plain(item.title).includes(text) ||
    (id !== "" && alphanumeric(plain(item.readableId)).includes(id))
  );
}

export function filterWork(items: WorkItem[], filters: WorkFilters, ctx: FilterContext) {
  return items.filter((item) => matches(item, filters, ctx, undefined));
}

/** Due date ascending, the items without one last, then the readable ID. The input stays as is. */
export function sortWork(items: WorkItem[]): WorkItem[] {
  return [...items].sort((a, b) => {
    const [dueA, dueB] = [dueOf(a), dueOf(b)];
    if (dueA !== dueB) {
      if (dueA === undefined) {
        return 1;
      }
      if (dueB === undefined) {
        return -1;
      }
      return dueA < dueB ? -1 : 1;
    }
    return a.readableId.localeCompare(b.readableId, "en", { numeric: true });
  });
}

export const SORT_KEYS = ["id", "kind", "title", "status", "owner", "due", "area"] as const;
export type SortKey = (typeof SORT_KEYS)[number];
export type SortDirection = "asc" | "desc";

/** How the table sorts. The default is the date, the soonest first. */
export interface Sort {
  key: SortKey;
  dir: SortDirection;
}

export const DEFAULT_SORT: Sort = { key: "due", dir: "asc" };

const READABLE = (a: string, b: string) => a.localeCompare(b, "en", { numeric: true });
const TEXT = (a: string, b: string) => a.localeCompare(b, "de-CH", { numeric: true });

/** What a column compares. Nothing means „no value“, which sorts last in both directions. */
function sortValue(
  item: WorkItem,
  key: SortKey,
  areaOf: (item: WorkItem) => string | undefined,
): string | undefined {
  const text = (value: string | undefined) => (value === "" ? undefined : value);
  switch (key) {
    case "id":
      return item.readableId;
    case "kind":
      return String(RECORD_KINDS.indexOf(item.kind));
    case "title":
      return text(item.title);
    case "status":
      return `${BOARD_COLUMNS.indexOf(item.column)}:${item.status}`;
    case "owner":
      return text(item.owner.name);
    case "due":
      // The column shows the day of a decision, so it sorts by it too.
      return item.date;
    case "area":
      return text(areaOf(item));
  }
}

/**
 * Sorts for the table: by one column, the records without a value last in both directions, a tie
 * by the readable ID. The sort is stable and the input stays as it is. `areaOf` names the
 * workstream of an item, which the item itself does not know.
 */
export function sortBy(
  items: WorkItem[],
  { key, dir }: Sort,
  areaOf: (item: WorkItem) => string | undefined,
): WorkItem[] {
  const compare =
    key === "id" || key === "due" || key === "kind" || key === "status" ? READABLE : TEXT;
  const sign = dir === "asc" ? 1 : -1;
  return [...items].sort((a, b) => {
    const [x, y] = [sortValue(a, key, areaOf), sortValue(b, key, areaOf)];
    if (x === undefined || y === undefined) {
      if (x !== y) {
        return x === undefined ? 1 : -1;
      }
    } else {
      const order = compare(x, y);
      if (order !== 0) {
        return sign * order;
      }
    }
    return READABLE(a.readableId, b.readableId);
  });
}

/** The items per board column, in the order of ADR 0082, each column sorted. */
export function groupByColumn(items: WorkItem[]): { column: BoardColumn; items: WorkItem[] }[] {
  const sorted = sortWork(items);
  return BOARD_COLUMNS.map((column) => ({
    column,
    items: sorted.filter((item) => item.column === column),
  }));
}

export interface Counts {
  kinds: Record<RecordKind, number>;
  columns: Record<BoardColumn, number>;
}

/**
 * The numbers beside the type filter and in the column heads. A kind counts the items that all other
 * filters let through, so that the filter shows what a click would add. A column counts the items
 * that every filter lets through, which is what the board shows.
 */
export function countsFor(items: WorkItem[], filters: WorkFilters, ctx: FilterContext): Counts {
  const kinds = Object.fromEntries(RECORD_KINDS.map((kind) => [kind, 0])) as Counts["kinds"];
  const columns = Object.fromEntries(BOARD_COLUMNS.map((c) => [c, 0])) as Counts["columns"];
  for (const item of items) {
    if (matches(item, filters, ctx, "kinds")) {
      kinds[item.kind] += 1;
      if (filters.kinds.length === 0 || filters.kinds.includes(item.kind)) {
        columns[item.column] += 1;
      }
    }
  }
  return { kinds, columns };
}

const KEYS = {
  kinds: "type",
  owners: "owner",
  workstreams: "stream",
  due: "due",
  text: "q",
} as const;
/** The query keys of the filters, so that a page can clear them all before it writes new ones. */
export const FILTER_KEYS: readonly string[] = Object.values(KEYS);
const ID = /^[\w-]{1,64}$/;
const MAX_TEXT = 200;

function listOf(params: Pick<URLSearchParams, "getAll">, key: string): string[] {
  return params
    .getAll(key)
    .flatMap((value) => value.split(","))
    .filter((value) => value !== "");
}

/** The known values, once each, in the order of the table. */
function inOrder<T extends string>(table: readonly T[], values: string[]): T[] {
  return table.filter((value) => values.includes(value));
}

function ids(values: string[]): string[] {
  return [...new Set(values.filter((value) => ID.test(value)))];
}

/** Reads the filters from a query string. What it does not know, it ignores. */
export function parseFilters(params: Pick<URLSearchParams, "get" | "getAll">): WorkFilters {
  const due = params.get(KEYS.due);
  return {
    kinds: inOrder(RECORD_KINDS, listOf(params, KEYS.kinds)),
    owners: ids(listOf(params, KEYS.owners)),
    workstreams: ids(listOf(params, KEYS.workstreams)),
    due: DUE_WINDOWS.find((window) => window === due),
    text: (params.get(KEYS.text) ?? "").trim().slice(0, MAX_TEXT),
  };
}

/** Writes the filters as a query string. A filter at its default has no key. */
export function serializeFilters(filters: WorkFilters): URLSearchParams {
  const params = new URLSearchParams();
  const lists = [
    [KEYS.kinds, filters.kinds],
    [KEYS.owners, filters.owners],
    [KEYS.workstreams, filters.workstreams],
  ] as const;
  for (const [key, values] of lists) {
    if (values.length > 0) {
      params.set(key, values.join(","));
    }
  }
  if (filters.due) {
    params.set(KEYS.due, filters.due);
  }
  if (filters.text.trim() !== "") {
    params.set(KEYS.text, filters.text.trim());
  }
  return params;
}

const SORT = "sort";
const DIR = "dir";
/** The query keys of the sort, so that a page can clear them before it writes new ones. */
export const SORT_PARAMS: readonly string[] = [SORT, DIR];

/** Reads the sort from a query string. What it does not know, it ignores. */
export function parseSort(params: Pick<URLSearchParams, "get">): Sort {
  const key = params.get(SORT);
  const dir = params.get(DIR);
  return {
    key: SORT_KEYS.find((candidate) => candidate === key) ?? DEFAULT_SORT.key,
    dir: dir === "asc" || dir === "desc" ? dir : DEFAULT_SORT.dir,
  };
}

/** Writes the sort as a query string. A part at its default has no key. */
export function serializeSort({ key, dir }: Sort): URLSearchParams {
  const params = new URLSearchParams();
  if (key !== DEFAULT_SORT.key) {
    params.set(SORT, key);
  }
  if (dir !== DEFAULT_SORT.dir) {
    params.set(DIR, dir);
  }
  return params;
}
