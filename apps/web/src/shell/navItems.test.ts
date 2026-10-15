import { describe, expect, it } from "vitest";
import { bottomItems, eventIdOf, isCurrent, mainItems, moreItems } from "./navItems";

const labels = (items: { label: string }[]) => items.map((item) => item.label);
const EVENT = "/events/e1";

describe("the bottom bar", () => {
  it("has Anlässe outside an event", () => {
    expect(labels(bottomItems(undefined))).toEqual(["Eingang", "Meine Arbeit", "Anlässe"]);
    expect(labels(moreItems(undefined))).toEqual(["Kontakte", "Einstellungen", "Datenschutz"]);
  });

  it("has Zeitplan after Arbeit inside an event and not outside", () => {
    expect(labels(mainItems("e1")).slice(2, 4)).toEqual(["Arbeit", "Zeitplan"]);
    expect(labels(mainItems(undefined))).not.toContain("Zeitplan");
    expect(mainItems("e1").find((item) => item.label === "Zeitplan")?.to).toBe(
      "/events/e1/work?view=timeline",
    );
  });

  it("has Dokumente after Zeitplan inside an event and not outside", () => {
    expect(labels(mainItems("e1")).slice(2, 5)).toEqual(["Arbeit", "Zeitplan", "Dokumente"]);
    expect(labels(mainItems(undefined))).not.toContain("Dokumente");
    expect(mainItems("e1").find((item) => item.label === "Dokumente")?.to).toBe(
      "/events/e1/documents",
    );
  });

  it("has Arbeit instead of Anlässe inside an event, and Anlässe and Dokumente move into Mehr", () => {
    expect(labels(bottomItems("e1"))).toEqual(["Eingang", "Meine Arbeit", "Arbeit"]);
    expect(labels(moreItems("e1"))).toEqual([
      "Anlässe",
      "Dokumente",
      "Kontakte",
      "Einstellungen",
      "Datenschutz",
    ]);
  });
});

describe("isCurrent", () => {
  const current = (items: ReturnType<typeof mainItems>, pathname: string, search = "") =>
    labels(items.filter((item) => isCurrent(item, pathname, search)));

  it("marks only Arbeit on the work page of an event", () => {
    expect(current(mainItems("e1"), `${EVENT}/work`)).toEqual(["Arbeit"]);
    expect(current(moreItems("e1"), `${EVENT}/work`)).toEqual([]);
  });

  it("marks Zeitplan and not Arbeit on the work page in the timeline view", () => {
    expect(current(mainItems("e1"), `${EVENT}/work`, "?view=timeline")).toEqual(["Zeitplan"]);
    expect(current(mainItems("e1"), `${EVENT}/work`, "?q=zelt&view=timeline&type=action")).toEqual([
      "Zeitplan",
    ]);
  });

  it("marks Arbeit and not Zeitplan in the other views", () => {
    expect(current(mainItems("e1"), `${EVENT}/work`, "?view=table")).toEqual(["Arbeit"]);
    expect(current(mainItems("e1"), `${EVENT}/work`, "?view=board&type=risk")).toEqual(["Arbeit"]);
  });

  it("marks neither Zeitplan nor Anlässe's sibling on other pages of the event", () => {
    expect(current(mainItems("e1"), EVENT, "?view=timeline")).toEqual(["Anlässe"]);
  });

  it("marks Anlässe on the overview of an event, and Arbeit not", () => {
    expect(current(mainItems("e1"), EVENT)).toEqual(["Anlässe"]);
    expect(current(moreItems("e1"), EVENT)).toEqual(["Anlässe"]);
    expect(current(bottomItems("e1"), EVENT)).toEqual([]);
  });

  it("marks Dokumente and not Anlässe on the documents of an event", () => {
    expect(current(mainItems("e1"), `${EVENT}/documents`)).toEqual(["Dokumente"]);
    expect(current(moreItems("e1"), `${EVENT}/documents`)).toEqual(["Dokumente"]);
  });

  it("marks Dokumente on a document and on its differences", () => {
    expect(current(mainItems("e1"), `${EVENT}/documents/d1`)).toEqual(["Dokumente"]);
    expect(current(mainItems("e1"), `${EVENT}/documents/d1/diff`, "?from=a&to=b")).toEqual([
      "Dokumente",
    ]);
  });

  it("marks Kontakte on both tabs of the page, and nothing else", () => {
    expect(current(mainItems(undefined), "/contacts")).toEqual(["Kontakte"]);
    expect(current(mainItems(undefined), "/contacts", "?tab=institutions")).toEqual(["Kontakte"]);
    expect(current(moreItems(undefined), "/contacts", "?tab=institutions")).toEqual(["Kontakte"]);
    expect(current(mainItems("e1"), "/contacts")).toEqual(["Kontakte"]);
  });

  it("marks Anlässe on the list of events", () => {
    expect(current(bottomItems(undefined), "/events")).toEqual(["Anlässe"]);
  });
});

describe("eventIdOf", () => {
  it("names the event of a document and of its differences", () => {
    expect(eventIdOf("/events/e1/documents/d1")).toBe("e1");
    expect(eventIdOf("/events/e1/documents/d1/diff")).toBe("e1");
  });
});
