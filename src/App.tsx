import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import { useFitWindowHeight } from "./useFitWindowHeight";

type SchedulerStatus = {
  nextPingAtUnix: number | null;
  pingMinMinutes: number;
  pingMaxMinutes: number;
  awaitingFollowup: boolean;
  plannedCheckSubject: string | null;
};

type CaptureRow = {
  body: string;
  createdAtUnix: number;
  durationMinutes: number | null;
  threadRoot: string;
};

type ActivityDigestRow = {
  threadRoot: string;
  totalMinutes: number;
  captureCount: number;
};

type MainTab = "capture" | "history" | "schedule";

type DigestPeriod = "day" | "week" | "month" | "all";

type QuickPickRow = {
  body: string;
  count: number;
};

const RECENT_CAPTURES_LIMIT = 15;

/** Matches `repo::PING_MAX_MINUTES_CAP` (one week). */
const PING_MAX_MINUTES_CAP = 10_080;

/** Planned / extra duration presets (minutes); custom values allowed in the input. */
const PLANNED_DURATION_PRESETS = [5, 15, 30, 60, 120, 240, 360] as const;

const DEFAULT_PLANNED_MINUTES = 30;

function minutesMatchPreset(input: string, minutes: number): boolean {
  const t = input.trim();
  if (t === "") return false;
  const n = Number.parseInt(t, 10);
  return Number.isFinite(n) && n === minutes;
}

/** Empty field → null planned duration (random / backend default next ping). */
function isUnsetPlannedMinutes(input: string): boolean {
  return input.trim() === "";
}

function digestSinceUnix(period: DigestPeriod): number {
  const now = Math.floor(Date.now() / 1000);
  switch (period) {
    case "day":
      return now - 86_400;
    case "week":
      return now - 7 * 86_400;
    case "month":
      return now - 30 * 86_400;
    case "all":
      return 0;
    default: {
      const _exhaustive: never = period;
      return _exhaustive;
    }
  }
}

function formatSegmentMinutes(m: number): string {
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  const r = m % 60;
  return r === 0 ? `${h} h` : `${h} h ${r} min`;
}

function formatNextPing(unix: number | null): string {
  if (unix == null) {
    return "Next ping: not scheduled yet.";
  }
  return `Next ping around ${new Date(unix * 1000).toLocaleString(undefined, {
    dateStyle: "short",
    timeStyle: "medium",
  })}`;
}

/** Matches `still_button_label` in `src-tauri/src/platform/windows.rs` (toast actions). */
function formatStillButtonLabel(body: string): string {
  const PREFIX = "Still: ";
  const MAX_CHARS = 42;
  const t = body.trim();
  if (!t) return "Still";
  const chars = Array.from(t);
  const prefixLen = Array.from(PREFIX).length;
  const avail = Math.max(0, MAX_CHARS - prefixLen);
  if (chars.length <= avail) return PREFIX + t;
  return PREFIX + chars.slice(0, Math.max(0, avail - 1)).join("") + "…";
}

/** Quick-capture shell. All user-facing strings are English until i18n (see /I18N.md). */
export default function App() {
  useFitWindowHeight();

  const labelId = useId();
  const quickPicksLegendId = useId();
  const plannedMinutesId = useId();
  const plannedPresetsLegendId = useId();
  const minId = useId();
  const maxId = useId();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState("");
  const [plannedMinutes, setPlannedMinutes] = useState(
    String(DEFAULT_PLANNED_MINUTES),
  );
  const [awaitingFollowup, setAwaitingFollowup] = useState(false);
  const [plannedCheckSubject, setPlannedCheckSubject] = useState<string | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [lastPingAt, setLastPingAt] = useState<string | null>(null);
  const [schedulerLine, setSchedulerLine] = useState<string | null>(null);
  const [intervalLine, setIntervalLine] = useState<string | null>(null);
  const [boundsMin, setBoundsMin] = useState(30);
  const [boundsMax, setBoundsMax] = useState(120);
  const [intervalError, setIntervalError] = useState<string | null>(null);
  const [applyingInterval, setApplyingInterval] = useState(false);
  const [snoozing, setSnoozing] = useState(false);
  const [repeating, setRepeating] = useState(false);
  const [recentCaptures, setRecentCaptures] = useState<CaptureRow[]>([]);
  const [quickPicks, setQuickPicks] = useState<QuickPickRow[]>([]);
  const [mainTab, setMainTab] = useState<MainTab>("capture");
  const [digestPeriod, setDigestPeriod] = useState<DigestPeriod>("week");
  const [digest, setDigest] = useState<ActivityDigestRow[]>([]);
  const [digestCopied, setDigestCopied] = useState(false);

  const latestCaptureBody = recentCaptures[0]?.body?.trim() ?? "";
  const canRepeatLast = latestCaptureBody.length > 0;

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

  const refreshQuickPicks = useCallback(async () => {
    try {
      const rows = await invoke<QuickPickRow[]>("list_top_quick_picks");
      setQuickPicks(rows);
    } catch {
      setQuickPicks([]);
    }
  }, []);

  const refreshCaptureLists = useCallback(async () => {
    await refreshRecentCaptures();
    await refreshQuickPicks();
  }, [refreshRecentCaptures, refreshQuickPicks]);

  const refreshDigest = useCallback(async () => {
    try {
      const sinceUnix = digestSinceUnix(digestPeriod);
      const rows = await invoke<ActivityDigestRow[]>("list_activity_digest", {
        sinceUnix,
      });
      setDigest(rows);
    } catch {
      setDigest([]);
    }
  }, [digestPeriod]);

  const refreshSchedulerStatus = useCallback(async () => {
    try {
      const s = await invoke<SchedulerStatus>("get_scheduler_status");
      setSchedulerLine(formatNextPing(s.nextPingAtUnix));
      setIntervalLine(
        `Random interval: ${s.pingMinMinutes}–${s.pingMaxMinutes} min`,
      );
      setBoundsMin(s.pingMinMinutes);
      setBoundsMax(s.pingMaxMinutes);
      setAwaitingFollowup(Boolean(s.awaitingFollowup));
      setPlannedCheckSubject(s.plannedCheckSubject ?? null);
    } catch {
      setSchedulerLine(null);
      setIntervalLine(null);
      setAwaitingFollowup(false);
      setPlannedCheckSubject(null);
    }
  }, []);

  useEffect(() => {
    void refreshSchedulerStatus();
    void refreshCaptureLists();
  }, [refreshSchedulerStatus, refreshCaptureLists]);

  useEffect(() => {
    if (mainTab !== "history") return;
    void refreshRecentCaptures();
    void refreshDigest();
  }, [mainTab, refreshRecentCaptures, refreshDigest]);

  useEffect(() => {
    if (mainTab !== "history") return;
    void refreshDigest();
  }, [digestPeriod, mainTab, refreshDigest]);

  const wasAwaitingFollowupRef = useRef(false);
  useEffect(() => {
    if (awaitingFollowup) {
      setPlannedMinutes("");
      if (!wasAwaitingFollowupRef.current) {
        setMainTab("capture");
      }
    }
    wasAwaitingFollowupRef.current = awaitingFollowup;
  }, [awaitingFollowup]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        if (cancelled) return;
        const offPingDue = await listen("ping-due", () => {
          setLastPingAt(
            new Date().toLocaleTimeString(undefined, {
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
            }),
          );
          void refreshSchedulerStatus();
          void refreshCaptureLists();
        });
        const offScheduler = await listen("scheduler-updated", () => {
          void refreshSchedulerStatus();
          void refreshCaptureLists();
        });
        unlisten = () => {
          offPingDue();
          offScheduler();
        };
      } catch {
        // `npm run dev` without Tauri — no event bridge.
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refreshSchedulerStatus, refreshCaptureLists]);

  async function onSnooze() {
    setSnoozing(true);
    try {
      await invoke<SchedulerStatus>("snooze_ping");
      void refreshSchedulerStatus();
    } finally {
      setSnoozing(false);
    }
  }

  async function onRepeatLast() {
    setError(null);
    setRepeating(true);
    try {
      await invoke<SchedulerStatus>("repeat_last_capture");
      void refreshSchedulerStatus();
      void refreshCaptureLists();
    } catch (err) {
      setError(String(err));
    } finally {
      setRepeating(false);
    }
  }

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

  async function copyDigestSummary() {
    if (digest.length === 0) return;
    const lines = digest.map((row) => {
      const time =
        row.totalMinutes > 0
          ? formatSegmentMinutes(row.totalMinutes)
          : "no logged minutes";
      return `${row.threadRoot}\t${time}\t(${row.captureCount} saves)`;
    });
    const header = `Period: ${digestPeriod} (segment totals sum when you set minutes)`;
    try {
      await navigator.clipboard.writeText(`${header}\n\n${lines.join("\n")}`);
      setDigestCopied(true);
      window.setTimeout(() => setDigestCopied(false), 2000);
    } catch {
      setDigestCopied(false);
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

    const rawMinutes = plannedMinutes.trim();
    let plannedDurationMinutes: number | null = null;
    if (rawMinutes !== "") {
      const n = Number.parseInt(rawMinutes, 10);
      if (!Number.isFinite(n) || n < 1 || n > PING_MAX_MINUTES_CAP) {
        setError(
          `Minutes must be between 1 and ${PING_MAX_MINUTES_CAP.toLocaleString()}.`,
        );
        return;
      }
      plannedDurationMinutes = n;
    }

    setSaving(true);
    try {
      await invoke("submit_capture", {
        text: trimmed,
        plannedDurationMinutes,
      });
      setText("");
      setPlannedMinutes(String(DEFAULT_PLANNED_MINUTES));
      void refreshSchedulerStatus();
      void refreshCaptureLists();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  const tabBtnClass = (active: boolean) =>
    `flex-1 rounded-lg px-3 py-2 text-sm font-medium transition-colors duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand ${
      active
        ? "bg-brand/18 text-ink shadow-sm"
        : "text-ink/60 hover:bg-brand/10 hover:text-ink/85"
    }`;

  return (
    <main className="mx-auto flex max-w-md flex-col gap-4 px-5 py-6">
      <nav
        className="flex gap-1 rounded-xl border border-brand/20 bg-white/70 p-1 shadow-sm"
        role="tablist"
        aria-label="Main sections"
      >
        <button
          type="button"
          role="tab"
          aria-selected={mainTab === "capture"}
          onClick={() => setMainTab("capture")}
          className={tabBtnClass(mainTab === "capture")}
        >
          Capture
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={mainTab === "history"}
          onClick={() => setMainTab("history")}
          className={tabBtnClass(mainTab === "history")}
        >
          History
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={mainTab === "schedule"}
          onClick={() => setMainTab("schedule")}
          className={tabBtnClass(mainTab === "schedule")}
        >
          Schedule
        </button>
      </nav>

      {mainTab === "capture" ? (
        <header className="space-y-0.5">
          <h1 className="text-lg font-semibold tracking-tight text-ink">
            What are you doing?
          </h1>
          <p className="text-sm text-ink/70">
            Quick capture — honest answer. Timing controls live on{" "}
            <button
              type="button"
              className="font-medium text-brand underline decoration-brand/35 underline-offset-2 hover:decoration-brand"
              onClick={() => setMainTab("schedule")}
            >
              Schedule
            </button>
            .
          </p>
        </header>
      ) : null}

      {mainTab === "schedule" ? (
        <section className="flex flex-col gap-4" aria-label="Ping schedule">
          <header className="space-y-1">
            <h1 className="text-lg font-semibold tracking-tight text-ink">
              Ping schedule
            </h1>
            <p className="text-xs text-ink/50">
              Closing the window hides it to the system tray; use the tray
              icon, menu, or tap the ping notification to show this window again.
            </p>
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
            {schedulerLine ? (
              <div className="pt-1">
                <button
                  type="button"
                  onClick={() => void onSnooze()}
                  disabled={snoozing}
                  className="cursor-pointer rounded-md border border-brand/30 bg-white/90 px-2.5 py-1 text-xs font-medium text-ink/90 transition-colors hover:bg-brand/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {snoozing ? "Snoozing…" : "Snooze 10 min"}
                </button>
                <p className="mt-1 text-[0.65rem] leading-snug text-ink/45">
                  Next ping moves to about ten minutes from now (saved; survives
                  restart). Random interval applies after that ping.
                </p>
              </div>
            ) : null}
          </header>

          <details className="rounded-lg border border-brand/20 bg-white/80 px-3 py-2 shadow-sm open:pb-3">
            <summary className="cursor-pointer select-none text-sm font-medium text-ink">
              Random ping interval
            </summary>
            <div className="mt-3 flex flex-col gap-3">
              <div className="grid grid-cols-2 gap-3">
                <div className="flex flex-col gap-1">
                  <label
                    htmlFor={minId}
                    className="text-xs font-medium text-ink/80"
                  >
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
                  <label
                    htmlFor={maxId}
                    className="text-xs font-medium text-ink/80"
                  >
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
        </section>
      ) : null}

      {mainTab === "history" ? (
        <section
          className="flex flex-col gap-5"
          aria-label="History and summaries"
        >
          <header>
            <h1 className="text-lg font-semibold tracking-tight text-ink">
              History
            </h1>
            <p className="mt-1 text-sm text-ink/65">
              Raw captures and rolled-up time by activity. Extra minutes on
              follow-up pings add to the same activity for charts later.
            </p>
          </header>

          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-2">
              <p className="text-xs font-medium uppercase tracking-wide text-ink/55">
                Summary
              </p>
              <button
                type="button"
                disabled={digest.length === 0}
                onClick={() => void copyDigestSummary()}
                className="cursor-pointer rounded-md border border-brand/30 bg-white px-2.5 py-1 text-xs font-medium text-ink/90 transition-colors hover:bg-brand/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-50"
              >
                {digestCopied ? "Copied" : "Copy summary"}
              </button>
            </div>
            <div
              className="flex flex-wrap gap-1.5"
              role="group"
              aria-label="Summary period"
            >
              {(
                [
                  ["day", "24h"],
                  ["week", "7d"],
                  ["month", "30d"],
                  ["all", "All"],
                ] as const
              ).map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  aria-pressed={digestPeriod === id}
                  onClick={() => setDigestPeriod(id)}
                  className={`cursor-pointer rounded-full border px-2.5 py-1 text-xs font-medium tabular-nums transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand ${
                    digestPeriod === id
                      ? "border-brand bg-brand/15 text-ink"
                      : "border-brand/25 bg-white/95 text-ink/80 hover:border-brand/35"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
            {digest.length === 0 ? (
              <p className="text-sm text-ink/55">
                No rows in this window yet — save captures with minutes to build
                totals.
              </p>
            ) : (
              <ul className="flex flex-col gap-1.5 rounded-lg border border-brand/15 bg-white/90 p-2 text-sm">
                {digest.map((row) => (
                  <li
                    key={row.threadRoot}
                    className="flex flex-wrap items-baseline justify-between gap-2 border-b border-brand/10 pb-1.5 last:border-b-0 last:pb-0"
                  >
                    <span className="min-w-0 flex-1 break-words text-ink">
                      {row.threadRoot}
                    </span>
                    <span className="shrink-0 tabular-nums text-xs text-ink/70">
                      {formatSegmentMinutes(row.totalMinutes)} ·{" "}
                      {row.captureCount} saves
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="flex flex-col gap-2">
            <p className="text-xs font-medium text-ink/65">
              Recent ({recentCaptures.length})
            </p>
            {recentCaptures.length === 0 ? (
              <p className="text-sm text-ink/60">Nothing saved yet.</p>
            ) : (
              <ul
                className="flex max-h-72 flex-col gap-2 overflow-y-auto text-sm"
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
                    <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-ink/50">
                      <span>
                        {new Date(row.createdAtUnix * 1000).toLocaleString(
                          undefined,
                          {
                            dateStyle: "short",
                            timeStyle: "short",
                          },
                        )}
                      </span>
                      {row.durationMinutes != null ? (
                        <span className="tabular-nums text-ink/45">
                          +{formatSegmentMinutes(row.durationMinutes)} segment
                        </span>
                      ) : null}
                      {row.threadRoot !== row.body ? (
                        <span className="text-ink/40">
                          · thread: {row.threadRoot}
                        </span>
                      ) : null}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>
      ) : null}

      {mainTab === "capture" ? (
      <form onSubmit={onSubmit} className="flex flex-col gap-3">
        {awaitingFollowup && plannedCheckSubject ? (
          <div
            role="status"
            className="rounded-lg border border-brand/35 bg-brand/10 px-3 py-2.5 text-sm text-ink/90 shadow-sm"
          >
            <p className="font-medium text-ink">Follow-up ping</p>
            <p className="mt-1 text-xs leading-snug text-ink/80">
              Planned time for “
              <span className="break-words">{plannedCheckSubject}</span>
              ” is up — update what you&apos;re doing.{" "}
              <strong>Extra minutes</strong> add another logged segment for that
              activity (stacked for History summaries); leave blank for a random
              next ping (
              <span className="tabular-nums">{boundsMin}</span>–
              <span className="tabular-nums">{boundsMax}</span> min).
            </p>
          </div>
        ) : null}

        <div className="flex flex-col gap-2">
          <label
            htmlFor={labelId}
            className="text-sm font-medium text-ink"
          >
            Right now
          </label>
          <div
            className={`flex flex-col overflow-hidden rounded-lg border border-brand/25 bg-white shadow-sm ring-brand/20 transition-shadow duration-interaction focus-within:border-brand focus-within:ring-[3px] ${
              saving || repeating ? "opacity-60" : ""
            }`}
          >
            <textarea
              ref={textareaRef}
              id={labelId}
              value={text}
              onChange={(e) => setText(e.target.value)}
              rows={5}
              disabled={saving || repeating}
              placeholder="Honest answer…"
              className="min-h-[7.5rem] w-full resize-y rounded-none border-0 bg-transparent px-3 py-2.5 text-sm text-ink outline-none placeholder:text-ink/40 focus:ring-0 disabled:cursor-not-allowed"
              aria-invalid={error ? true : undefined}
              aria-describedby={
                [
                  error ? `${labelId}-err` : null,
                  quickPicks.length > 0 ? quickPicksLegendId : null,
                ]
                  .filter(Boolean)
                  .join(" ") || undefined
              }
            />
            {quickPicks.length > 0 ? (
              <div
                className="border-t border-brand/15 bg-brand/[0.04] px-2 pb-2 pt-1.5"
                role="group"
                aria-labelledby={quickPicksLegendId}
              >
                <p
                  id={quickPicksLegendId}
                  className="mb-1 text-[0.65rem] font-medium uppercase tracking-wide text-ink/45"
                >
                  Common answers
                </p>
                <div className="flex flex-wrap gap-1.5">
                  {quickPicks.map((pick) => (
                    <button
                      key={pick.body}
                      type="button"
                      disabled={saving || repeating}
                      title={pick.body}
                      aria-label={`Insert quick answer: ${pick.body}`}
                      onClick={() => {
                        setError(null);
                        setText(pick.body);
                        requestAnimationFrame(() =>
                          textareaRef.current?.focus(),
                        );
                      }}
                      className="inline-flex max-w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-full border border-brand/20 bg-white/90 py-0.5 pl-2 pr-1.5 text-left text-[0.7rem] font-medium text-ink/90 shadow-sm transition-colors duration-200 hover:border-brand/35 hover:bg-brand/8 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60"
                    >
                      <span className="min-w-0 flex-1 truncate">{pick.body}</span>
                      <span
                        className="shrink-0 tabular-nums text-[0.6rem] font-normal text-ink/40"
                        aria-hidden
                      >
                        ({pick.count})
                      </span>
                    </button>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        </div>

        <div className="flex flex-col gap-2">
          <label
            htmlFor={plannedMinutesId}
            className="text-xs font-medium text-ink/75"
          >
            {awaitingFollowup
              ? "Extra minutes if still on this (optional)"
              : "Plan to keep at this for about (minutes)"}
          </label>
          <p
            id={plannedPresetsLegendId}
            className="text-[0.65rem] leading-snug text-ink/45"
          >
            {awaitingFollowup
              ? 'Presets or custom minutes; use “No idea” or leave empty for your random interval next.'
              : `Presets below — “No idea” clears the plan (random next ping); default ${DEFAULT_PLANNED_MINUTES} min, or type 1–${PING_MAX_MINUTES_CAP.toLocaleString()}.`}
          </p>
          <div
            role="group"
            aria-labelledby={plannedPresetsLegendId}
            className="flex flex-wrap gap-2"
          >
            <button
              type="button"
              disabled={saving || repeating}
              title="No fixed duration — next ping uses random interval"
              aria-label="No idea — clear planned minutes"
              aria-pressed={isUnsetPlannedMinutes(plannedMinutes)}
              onClick={() => setPlannedMinutes("")}
              className={`cursor-pointer rounded-full border px-2.5 py-1 text-xs font-medium transition-colors duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60 ${
                isUnsetPlannedMinutes(plannedMinutes)
                  ? "border-brand bg-brand/15 text-ink"
                  : "border-brand/25 bg-white/95 text-ink/90 hover:border-brand/40 hover:bg-brand/8"
              }`}
            >
              No idea
            </button>
            {PLANNED_DURATION_PRESETS.map((m) => {
              const active = minutesMatchPreset(plannedMinutes, m);
              return (
                <button
                  key={m}
                  type="button"
                  disabled={saving || repeating}
                  title={`${m} minutes`}
                  aria-label={`Set planned duration to ${m} minutes`}
                  aria-pressed={active}
                  onClick={() => setPlannedMinutes(String(m))}
                  className={`cursor-pointer rounded-full border px-2.5 py-1 text-xs font-medium tabular-nums transition-colors duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60 ${
                    active
                      ? "border-brand bg-brand/15 text-ink"
                      : "border-brand/25 bg-white/95 text-ink/90 hover:border-brand/40 hover:bg-brand/8"
                  }`}
                >
                  {m}
                </button>
              );
            })}
          </div>
          <input
            id={plannedMinutesId}
            type="number"
            min={1}
            max={PING_MAX_MINUTES_CAP}
            inputMode="numeric"
            value={plannedMinutes}
            onChange={(e) => setPlannedMinutes(e.target.value)}
            disabled={saving || repeating}
            placeholder={
              awaitingFollowup
                ? `Blank → next ping in ${boundsMin}–${boundsMax} min`
                : "Custom minutes (or clear for random next ping)"
            }
            className="rounded-md border border-brand/25 bg-white px-2.5 py-2 text-sm text-ink outline-none ring-brand/15 transition-colors duration-200 placeholder:text-ink/38 focus:border-brand focus:ring-2 disabled:cursor-not-allowed disabled:opacity-60"
          />
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="submit"
            disabled={saving || repeating}
            className="cursor-pointer rounded-lg bg-action px-4 py-2.5 text-sm font-semibold text-white shadow-sm transition-colors duration-interaction hover:bg-action-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-action motion-safe:active:scale-[0.99] disabled:cursor-not-allowed disabled:opacity-60"
          >
            {saving ? "Saving…" : "Save"}
          </button>
          <button
            type="button"
            onClick={() => void onRepeatLast()}
            disabled={saving || repeating || !canRepeatLast}
            title={
              canRepeatLast
                ? "Log the same answer as your last save (no need to retype)."
                : "Save an answer once to enable this."
            }
            className="cursor-pointer rounded-lg border border-brand/35 bg-white/90 px-4 py-2.5 text-sm font-medium text-ink/90 shadow-sm transition-colors duration-interaction hover:bg-brand/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60"
          >
            {repeating
              ? "Saving…"
              : formatStillButtonLabel(latestCaptureBody)}
          </button>
        </div>

        <div
          id={`${labelId}-err`}
          role="status"
          aria-live="polite"
          className="min-h-[1.25rem] text-sm font-medium text-red-600"
        >
          {error ?? ""}
        </div>
      </form>
      ) : null}
    </main>
  );
}
