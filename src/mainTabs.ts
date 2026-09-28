export const MAIN_TABS = ["capture", "history", "schedule", "routines"] as const;

export type MainTab = (typeof MAIN_TABS)[number];

export type TabStep = "prev" | "next";

export function tabIndex(tab: MainTab): number {
  return MAIN_TABS.indexOf(tab);
}

function stepDelta(step: TabStep): number {
  switch (step) {
    case "next":
      return 1;
    case "prev":
      return -1;
    default: {
      const exhaustive: never = step;
      return exhaustive;
    }
  }
}

export function adjacentTab(tab: MainTab, step: TabStep): MainTab | null {
  const index = tabIndex(tab) + stepDelta(step);
  if (index < 0 || index >= MAIN_TABS.length) return null;
  return MAIN_TABS[index] ?? null;
}
