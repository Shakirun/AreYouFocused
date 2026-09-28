import { describe, expect, it } from "vitest";

import { resolveSwipeTab, swipeOffsetPx } from "./swipeTabGesture";

describe("resolveSwipeTab", () => {
  it("treats a leftward swipe as the next tab", () => {
    expect(resolveSwipeTab(-80, 10)).toBe("next");
  });

  it("treats a rightward swipe as the previous tab", () => {
    expect(resolveSwipeTab(80, 10)).toBe("prev");
  });

  it("ignores a vertical swipe", () => {
    expect(resolveSwipeTab(10, 80)).toBeNull();
    expect(resolveSwipeTab(-10, -80)).toBeNull();
  });

  it("ignores a swipe that is not clearly horizontal", () => {
    expect(resolveSwipeTab(80, 80)).toBeNull();
    expect(resolveSwipeTab(-60, 60)).toBeNull();
  });

  it("ignores a short horizontal move", () => {
    expect(resolveSwipeTab(48, 0)).toBeNull();
    expect(resolveSwipeTab(-20, 4)).toBeNull();
  });

  it("commits once horizontal travel passes the threshold", () => {
    expect(resolveSwipeTab(-49, 10)).toBe("next");
    expect(resolveSwipeTab(49, 0)).toBe("prev");
  });
});

describe("swipeOffsetPx", () => {
  it("follows the finger when a neighbor exists", () => {
    expect(swipeOffsetPx(-30, true, true)).toBe(-30);
    expect(swipeOffsetPx(40, true, true)).toBe(40);
  });

  it("does not drag past the first or last tab", () => {
    expect(swipeOffsetPx(40, false, true)).toBe(0);
    expect(swipeOffsetPx(-40, true, false)).toBe(0);
  });
});
