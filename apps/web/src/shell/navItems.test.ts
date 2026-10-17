import { describe, expect, it } from "vitest";
import {
  bottomItems,
  eventIdOf,
  eventItems,
  isCurrent,
  moreItems,
  ORGANIZATION_ITEMS,
} from "./navItems";

const labels = (items: { label: string }[]) => items.map((item) => item.label);
const EVENT = "/events/e1";

describe("the menu", () => {
  it("has the organization items, whatever the event", () => {
    expect(labels(ORGANIZATION_ITEMS)).toEqual(["Eingang", "Meine Arbeit", "Kontakte"]);
  });

  it("has the four event items of the scoped event, with their links", () => {
    expect(labels(eventItems("e1"))).toEqual(["Übersicht", "Arbeit", "Zeitplan", "Dokumente"]);
    expect(eventItems("e1").map((item) => item.to)).toEqual([
      "/events/e1",
      "/events/e1/work",
      "/events/e1/work?view=timeline",
      "/events/e1/documents",
    ]);
  });
});

describe("the bottom bar", () => {
  it("has Alle Anlässe outside an event", () => {
    expect(labels(bottomItems(undefined))).toEqual(["Eingang", "Meine Arbeit", "Alle Anlässe"]);
    expect(labels(moreItems(undefined))).toEqual(["Kontakte", "Einstellungen", "Datenschutz"]);
  });

  it("has Arbeit inside an event, and Übersicht and Dokumente move into Mehr", () => {
    expect(labels(bottomItems("e1"))).toEqual(["Eingang", "Meine Arbeit", "Arbeit"]);
    expect(labels(moreItems("e1"))).toEqual([
      "Übersicht",
      "Dokumente",
      "Kontakte",
      "Einstellungen",
      "Datenschutz",
    ]);
  });
});

describe("isCurrent", () => {
  const current = (items: ReturnType<typeof eventItems>, pathname: string, search = "") =>
    labels(items.filter((item) => isCurrent(item, pathname, search)));

  it("marks only Arbeit on the work page of an event", () => {
    expect(current(eventItems("e1"), `${EVENT}/work`)).toEqual(["Arbeit"]);
    expect(current(moreItems("e1"), `${EVENT}/work`)).toEqual([]);
  });

  it("marks Zeitplan and not Arbeit on the work page in the timeline view", () => {
    expect(current(eventItems("e1"), `${EVENT}/work`, "?view=timeline")).toEqual(["Zeitplan"]);
    expect(current(eventItems("e1"), `${EVENT}/work`, "?q=zelt&view=timeline&type=action")).toEqual(
      ["Zeitplan"],
    );
  });

  it("marks Arbeit and not Zeitplan in the other views", () => {
    expect(current(eventItems("e1"), `${EVENT}/work`, "?view=table")).toEqual(["Arbeit"]);
    expect(current(eventItems("e1"), `${EVENT}/work`, "?view=board&type=risk")).toEqual(["Arbeit"]);
  });

  it("marks Übersicht only on the exact path of the event", () => {
    expect(current(eventItems("e1"), EVENT)).toEqual(["Übersicht"]);
    expect(current(eventItems("e1"), EVENT, "?view=timeline")).toEqual(["Übersicht"]);
    expect(current(moreItems("e1"), EVENT)).toEqual(["Übersicht"]);
    expect(current(bottomItems("e1"), EVENT)).toEqual([]);
  });

  it("marks Dokumente and not Übersicht on the documents of an event", () => {
    expect(current(eventItems("e1"), `${EVENT}/documents`)).toEqual(["Dokumente"]);
    expect(current(moreItems("e1"), `${EVENT}/documents`)).toEqual(["Dokumente"]);
  });

  it("marks Dokumente on a document and on its differences", () => {
    expect(current(eventItems("e1"), `${EVENT}/documents/d1`)).toEqual(["Dokumente"]);
    expect(current(eventItems("e1"), `${EVENT}/documents/d1/diff`, "?from=a&to=b")).toEqual([
      "Dokumente",
    ]);
  });

  it("marks Kontakte on both tabs of the page, and nothing else", () => {
    expect(current(ORGANIZATION_ITEMS, "/contacts")).toEqual(["Kontakte"]);
    expect(current(ORGANIZATION_ITEMS, "/contacts", "?tab=institutions")).toEqual(["Kontakte"]);
    expect(current(moreItems(undefined), "/contacts", "?tab=institutions")).toEqual(["Kontakte"]);
  });

  it("marks no event item and no organization item on the list of events", () => {
    expect(current(eventItems("e1"), "/events")).toEqual([]);
    expect(current(ORGANIZATION_ITEMS, "/events")).toEqual([]);
    expect(current(bottomItems(undefined), "/events")).toEqual(["Alle Anlässe"]);
  });
});

describe("eventIdOf", () => {
  it("names the event of a document and of its differences", () => {
    expect(eventIdOf("/events/e1/documents/d1")).toBe("e1");
    expect(eventIdOf("/events/e1/documents/d1/diff")).toBe("e1");
  });
});
