import { describe, expect, it } from "vitest";
import { bottomItems, isCurrent, mainItems, moreItems } from "./navItems";

const labels = (items: { label: string }[]) => items.map((item) => item.label);
const EVENT = "/events/e1";

describe("the bottom bar", () => {
  it("has Anlässe outside an event", () => {
    expect(labels(bottomItems(undefined))).toEqual(["Eingang", "Meine Arbeit", "Anlässe"]);
    expect(labels(moreItems(undefined))).toEqual([
      "Personen",
      "Institutionen",
      "Einstellungen",
      "Datenschutz",
    ]);
  });

  it("has Arbeit instead of Anlässe inside an event, and Anlässe moves into Mehr", () => {
    expect(labels(bottomItems("e1"))).toEqual(["Eingang", "Meine Arbeit", "Arbeit"]);
    expect(labels(moreItems("e1"))).toEqual([
      "Anlässe",
      "Personen",
      "Institutionen",
      "Einstellungen",
      "Datenschutz",
    ]);
  });
});

describe("isCurrent", () => {
  const current = (items: ReturnType<typeof mainItems>, pathname: string) =>
    labels(items.filter((item) => isCurrent(item, pathname)));

  it("marks only Arbeit on the work page of an event", () => {
    expect(current(mainItems("e1"), `${EVENT}/work`)).toEqual(["Arbeit"]);
    expect(current(moreItems("e1"), `${EVENT}/work`)).toEqual([]);
  });

  it("marks Anlässe on the other pages of an event, and Arbeit not", () => {
    expect(current(mainItems("e1"), `${EVENT}/actions`)).toEqual(["Anlässe"]);
    expect(current(moreItems("e1"), `${EVENT}/actions`)).toEqual(["Anlässe"]);
    expect(current(bottomItems("e1"), `${EVENT}/actions`)).toEqual([]);
  });

  it("marks Anlässe on the list of events", () => {
    expect(current(bottomItems(undefined), "/events")).toEqual(["Anlässe"]);
  });
});
