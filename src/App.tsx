import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useId, useState } from "react";

type SchedulerStatus = {
  nextPingAtUnix: number | null;
  pingMinMinutes: number;
  pingMaxMinutes: number;
};

type CaptureRow = {
  body: string;
  createdAtUnix: number;
};

const RECENT_CAPTURES_LIMIT = 15;

/** Matches `repo::PING_MAX_MINUTES_CAP` (one week). */
const PING_MAX_MINUTES_CAP = 10_080;

function formatNextPing(unix: number | null): string {
  if (unix == null) {
    return "Next ping: not scheduled yet.";
  }
  return `Next ping around ${new Date(unix * 1000).toLocaleString(undefined, {
    dateStyle: "short",
    timeStyle: "medium",
  })}`;
}

/** Quick-capture shell. All user-facing strings are English until i18n (see /I18N.md). */
export default function App() {
  const labelId = useId();
  const minId = useId();
  const maxId = useId();
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [lastPingAt, setLastPingAt] = useState<string | null>(null);
  const [schedulerLine, setSchedulerLine] = useState<string | null>(null);
  const [intervalLine, setIntervalLine] = useState<string | null>(null);
  const [boundsMin, setBoundsMin] = useState(30);
  const [boundsMax, setBoundsMax] = useState(120);
  const [intervalError, setIntervalError] = useState<string | null>(null);
  const [applyingInterval, setApplyingInterval] = useState(false);
  const [recentCaptures, setRecentCaptures] = useState<CaptureRow[]>([]);

  const refreshRecentCaptures = useCallback(async () => {
    try {
      const rows = await invoke<CaptureRow[]>("list_recent_captures", {
        limit: RECENT_CAPTURES_LIMIT,
      });
      setRecentCaptures(rows);
    } catch {
      setRecentCaptures([]);
    }
  }, []);

  const refreshSchedulerStatus = useCallback(async () => {
    try {
      const s = await invoke<SchedulerStatus>("get_scheduler_status");
      setSchedulerLine(formatNextPing(s.nextPingAtUnix));
      setIntervalLine(
        `Random interval: ${s.pingMinMinutes}–${s.pingMaxMinutes} min`,
      );
      setBoundsMin(s.pingMinMinutes);
      setBoundsMax(s.pingMaxMinutes);
    } catch {
      setSchedulerLine(null);
      setIntervalLine(null);
    }
  }, []);

  useEffect(() => {
    void refreshSchedulerStatus();
    void refreshRecentCaptures();
  }, [refreshSchedulerStatus, refreshRecentCaptures]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        if (cancelled) return;
        unlisten = await listen("ping-due", () => {
          setLastPingAt(
            new Date().toLocaleTimeString(undefined, {
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
            }),
          );
          void refreshSchedulerStatus();
        });
      } catch {
        // `npm run dev` without Tauri — no event bridge.
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refreshSchedulerStatus]);

  async function onApplyInterval() {
    setIntervalError(null);
    if (boundsMin < 1) {
      setIntervalError("Minimum must be at least 1 minute.");
      return;
    }
    if (boundsMax < boundsMin) {
      setIntervalError("Maximum must be greater than or equal to minimum.");
      return;
    }
    if (boundsMax > PING_MAX_MINUTES_CAP) {
      setIntervalError(
        `Maximum must be at most ${PING_MAX_MINUTES_CAP} minutes (one week).`,
      );
      return;
    }
    setApplyingInterval(true);
    try {
      await invoke<SchedulerStatus>("update_ping_interval", {
        pingMinMinutes: boundsMin,
        pingMaxMinutes: boundsMax,
      });
      void refreshSchedulerStatus();
    } catch (err) {
      setIntervalError(String(err));
    } finally {
      setApplyingInterval(false);
    }
  }

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const trimmed = text.trim();
    if (!trimmed) {
      setError("Write something first.");
      return;
    }
    setSaving(true);
    try {
      await invoke("submit_capture", { text: trimmed });
      setText("");
      void refreshSchedulerStatus();
      void refreshRecentCaptures();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <main className="mx-auto flex min-h-screen max-w-md flex-col gap-6 px-5 py-8">
      <header className="space-y-1">
        <h1 className="text-lg font-semibold tracking-tight text-ink">
          What are you doing?
        </h1>
        <p className="text-sm text-ink/70">Quick capture — honest answer.</p>
        {lastPingAt ? (
          <p className="text-xs text-ink/55" aria-live="polite">
            Last ping at {lastPingAt}
          </p>
        ) : null}
        {schedulerLine ? (
          <p className="text-xs text-ink/55" aria-live="polite">
            {schedulerLine}
          </p>
        ) : null}
        {intervalLine ? (
          <p className="text-xs text-ink/45" aria-live="polite">
            {intervalLine}
          </p>
        ) : null}
      </header>

      <details className="rounded-lg border border-brand/20 bg-white/80 px-3 py-2 shadow-sm open:pb-3">
        <summary className="cursor-pointer select-none text-sm font-medium text-ink">
          Ping interval
        </summary>
        <div className="mt-3 flex flex-col gap-3">
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1">
              <label htmlFor={minId} className="text-xs font-medium text-ink/80">
                Min (minutes)
              </label>
              <input
                id={minId}
                type="number"
                min={1}
                max={PING_MAX_MINUTES_CAP}
                value={boundsMin}
                onChange={(e) =>
                  setBoundsMin(Number.parseInt(e.target.value, 10) || 0)
                }
                disabled={applyingInterval}
                className="rounded-md border border-brand/25 px-2 py-1.5 text-sm text-ink outline-none ring-brand/15 focus:border-brand focus:ring-2 disabled:opacity-60"
              />
            </div>
            <div className="flex flex-col gap-1">
              <label htmlFor={maxId} className="text-xs font-medium text-ink/80">
                Max (minutes)
              </label>
              <input
                id={maxId}
                type="number"
                min={1}
                max={PING_MAX_MINUTES_CAP}
                value={boundsMax}
                onChange={(e) =>
                  setBoundsMax(Number.parseInt(e.target.value, 10) || 0)
                }
                disabled={applyingInterval}
                className="rounded-md border border-brand/25 px-2 py-1.5 text-sm text-ink outline-none ring-brand/15 focus:border-brand focus:ring-2 disabled:opacity-60"
              />
            </div>
          </div>
          <button
            type="button"
            onClick={() => void onApplyInterval()}
            disabled={applyingInterval}
            className="self-start rounded-md border border-brand/30 bg-brand/10 px-3 py-1.5 text-sm font-medium text-ink transition-colors hover:bg-brand/15 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60"
          >
            {applyingInterval ? "Applying…" : "Apply interval"}
          </button>
          {intervalError ? (
            <p className="text-sm font-medium text-red-600" role="status">
              {intervalError}
            </p>
          ) : null}
        </div>
      </details>

      <form onSubmit={onSubmit} className="flex flex-col gap-3">
        <div className="flex flex-col gap-2">
          <label
            htmlFor={labelId}
            className="text-sm font-medium text-ink"
          >
            Right now
          </label>
          <textarea
            id={labelId}
            value={text}
            onChange={(e) => setText(e.target.value)}
            rows={5}
            disabled={saving}
            placeholder="Honest answer…"
            className="w-full resize-y rounded-lg border border-brand/25 bg-white px-3 py-2.5 text-sm text-ink shadow-sm outline-none ring-brand/20 transition-shadow duration-interaction placeholder:text-ink/40 focus:border-brand focus:ring-[3px] disabled:cursor-not-allowed disabled:opacity-60"
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? `${labelId}-err` : undefined}
          />
        </div>

        <button
          type="submit"
          disabled={saving}
          className="cursor-pointer rounded-lg bg-action px-4 py-2.5 text-sm font-semibold text-white shadow-sm transition-colors duration-interaction hover:bg-action-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-action motion-safe:active:scale-[0.99] disabled:cursor-not-allowed disabled:opacity-60"
        >
          {saving ? "Saving…" : "Save"}
        </button>

        <div
          id={`${labelId}-err`}
          role="status"
          aria-live="polite"
          className="min-h-[1.25rem] text-sm font-medium text-red-600"
        >
          {error ?? ""}
        </div>
      </form>

      <details className="rounded-lg border border-brand/20 bg-white/80 px-3 py-2 shadow-sm open:pb-3">
        <summary className="cursor-pointer select-none text-sm font-medium text-ink">
          Recent answers ({recentCaptures.length})
        </summary>
        {recentCaptures.length === 0 ? (
          <p className="mt-3 text-sm text-ink/60">Nothing saved yet.</p>
        ) : (
          <ul
            className="mt-3 flex max-h-64 flex-col gap-2 overflow-y-auto text-sm"
            aria-label="Recent capture entries"
          >
            {recentCaptures.map((row, i) => (
              <li
                key={`${row.createdAtUnix}-${i}`}
                className="rounded-md border border-brand/15 bg-white px-2.5 py-2"
              >
                <p className="line-clamp-4 whitespace-pre-wrap break-words text-ink">
                  {row.body}
                </p>
                <p className="mt-1 text-xs text-ink/50">
                  {new Date(row.createdAtUnix * 1000).toLocaleString(undefined, {
                    dateStyle: "short",
                    timeStyle: "short",
                  })}
                </p>
              </li>
            ))}
          </ul>
        )}
      </details>
    </main>
  );
}
