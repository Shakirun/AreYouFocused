import { useEffect, useRef } from "react";

/** Fixed inner width for the non-resizable capture window (`tauri.conf.json`). */
const WINDOW_INNER_WIDTH = 420;
const MIN_INNER_HEIGHT = 320;
/** Avoid a persistent 1px scrollbar from rounding / shadows. */
const HEIGHT_SLOP_PX = 12;

/**
 * Resizes the Tauri window inner height to match document content (`scrollHeight`).
 * No-op in plain browser (`vite dev` without Tauri IPC).
 */
export function useFitWindowHeight() {
  const lastApplied = useRef(0);
  const debounceTimer = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    let cancelled = false;

    async function apply() {
      if (cancelled) return;
      try {
        const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
        const scrollH = document.documentElement.scrollHeight;
        const maxH = Math.max(
          MIN_INNER_HEIGHT,
          Math.floor(window.screen.availHeight * 0.94),
        );
        const target = Math.min(
          maxH,
          Math.max(MIN_INNER_HEIGHT, Math.ceil(scrollH) + HEIGHT_SLOP_PX),
        );
        if (Math.abs(target - lastApplied.current) < 4) return;
        lastApplied.current = target;
        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(WINDOW_INNER_WIDTH, target));
      } catch {
        /* Not inside Tauri webview */
      }
    }

    function schedule() {
      clearTimeout(debounceTimer.current);
      debounceTimer.current = setTimeout(() => void apply(), 48);
    }

    const ro = new ResizeObserver(schedule);
    ro.observe(document.documentElement);

    void document.fonts?.ready?.then(() => schedule());

    requestAnimationFrame(() => {
      requestAnimationFrame(() => void apply());
    });

    return () => {
      cancelled = true;
      clearTimeout(debounceTimer.current);
      ro.disconnect();
    };
  }, []);
}
