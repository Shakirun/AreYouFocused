import { describe, expect, it } from "vitest";

import { MAIN_TABS, adjacentTab, tabIndex } from "./mainTabs";

describe("main tabs", () => {
  it("orders capture, history, schedule, then routines", () => {
    expect(MAIN_TABS).toEqual(["capture", "history", "schedule", "routines"]);
  });

  it("returns the index of a tab", () => {
    expect(tabIndex("capture")).toBe(0);
    expect(tabIndex("history")).toBe(1);
    expect(tabIndex("schedule")).toBe(2);
    expect(tabIndex("routines")).toBe(3);
  });

  it("does not wrap past the first or last tab", () => {
    expect(adjacentTab("capture", "prev")).toBeNull();
    expect(adjacentTab("routines", "next")).toBeNull();
  });

  it("steps to the neighbor in each direction", () => {
    expect(adjacentTab("capture", "next")).toBe("history");
    expect(adjacentTab("history", "prev")).toBe("capture");
    expect(adjacentTab("history", "next")).toBe("schedule");
    expect(adjacentTab("schedule", "next")).toBe("routines");
    expect(adjacentTab("routines", "prev")).toBe("schedule");
  });
});
