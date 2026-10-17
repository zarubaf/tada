import { describe, expect, it } from "vitest";
import { eventAddress } from "./switchEvent";

describe("eventAddress", () => {
  it.each([
    ["the work page", "/events/a/work", null, "/events/b/work"],
    [
      "the work page with the timeline view",
      "/events/a/work",
      "timeline",
      "/events/b/work?view=timeline",
    ],
    ["the documents", "/events/a/documents", null, "/events/b/documents"],
    ["a document", "/events/a/documents/d1", null, "/events/b/documents"],
    ["the diff of a document", "/events/a/documents/d1/diff", null, "/events/b/documents"],
    ["Übersicht", "/events/a", null, "/events/b"],
    ["an unknown page of the Anlass", "/events/a/unknown", null, "/events/b"],
    ["a page of the organization", "/inbox", null, "/events/b"],
    ["the start page", "/", null, "/events/b"],
    ["the list of Anlässe", "/events", null, "/events/b"],
    ["the form for a new Anlass", "/events/new", null, "/events/b"],
  ])("from %s", (_name, pathname, view, expected) => {
    expect(eventAddress(pathname, view, "b")).toBe(expected);
  });

  it("encodes the ID of the Anlass", () => {
    expect(eventAddress("/events/a/work", null, "b/c")).toBe("/events/b%2Fc/work");
  });

  it.each([["garbage"], ["board"], [""]])(
    "drops the view %j that is unknown or the default",
    (view) => {
      expect(eventAddress("/events/a/work", view, "b")).toBe("/events/b/work");
    },
  );

  it.each([["table"], ["timeline"]])("keeps the view %s", (view) => {
    expect(eventAddress("/events/a/work", view, "b")).toBe(`/events/b/work?view=${view}`);
  });
});
