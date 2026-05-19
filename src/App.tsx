import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import {
  DAILY_REMINDER_TIME_PRESETS,
  SLEEP_END_PRESETS,
  SLEEP_START_PRESETS,
  TimePickerField,
} from "./TimePickerField";
import { useFitWindowHeight } from "./useFitWindowHeight";

type SchedulerStatus = {
  nextPingAtUnix: number | null;
  pingMinMinutes: number;
  pingMaxMinutes: number;
  randomPingEnabled: boolean;
  overduePingEnabled: boolean;
  overduePingMinMinutes: number;
  overduePingMaxMinutes: number;
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

type ExportRangePreset = "today" | "this_week" | "last_7" | "custom";

type ExportHistoryResult = {
  saved: boolean;
  path?: string | null;
};

type QuickPickRow = {
  body: string;
  count: number;
};

type DailyReminderRow = {
  id: number;
  label: string;
  times: string[];
  enabled: boolean;
  burstCount: number;
  burstIntervalMin: number;
  presetKey: string | null;
  pillNote: string | null;
};

type DailyReminderSettings = {
  enabled: boolean;
  reminders: DailyReminderRow[];
};

type DailyReminderPreset = {
  key: string;
  label: string;
};

type SleepHoursSettings = {
  enabled: boolean;
  startHm: string;
  endHm: string;
};

type DailyReminderDraft = {
  id: number | null;
  presetKey: string;
  customLabel: string;
  pillNote: string;
  times: string[];
  enabled: boolean;
  burstCount: number;
  burstIntervalMin: number;
};

type CurrentActivityStatus = {
  body: string | null;
  startedAtUnix: number | null;
  durationMinutes: number | null;
  plannedEndAtUnix: number | null;
};

/** Emitted after −15 min shortens the last logged segment (`gap-fill-needed`). */
type ShortenGapPrompt = {
  adjustedEndUnix: number;
  gapMinutes: number;
  suggestedActivity: string;
};

const RECENT_CAPTURES_LIMIT = 15;

/** Matches `repo::PING_MAX_MINUTES_CAP` (one week). */
const PING_MAX_MINUTES_CAP = 10_080;

/** Planned / extra duration presets (minutes); custom values allowed in the input. */
const PLANNED_DURATION_PRESETS = [5, 15, 30, 60, 120, 240, 360] as const;

const DEFAULT_PLANNED_MINUTES = 30;

const DEFAULT_DAILY_BURST_COUNT = 3;
const DEFAULT_DAILY_BURST_INTERVAL_MIN = 5;

function emptyDailyDraft(): DailyReminderDraft {
  return {
    id: null,
    presetKey: "water",
    customLabel: "",
    pillNote: "",
    times: ["09:00"],
    enabled: true,
    burstCount: DEFAULT_DAILY_BURST_COUNT,
    burstIntervalMin: DEFAULT_DAILY_BURST_INTERVAL_MIN,
  };
}

function draftLabelFromPreset(
  draft: DailyReminderDraft,
  presets: DailyReminderPreset[],
): string {
  if (draft.presetKey === "custom") {
    return draft.customLabel.trim();
  }
  const p = presets.find((x) => x.key === draft.presetKey);
  return p?.label ?? draft.customLabel.trim();
}

function minutesMatchPreset(input: string, minutes: number): boolean {
  const t = input.trim();
  if (t === "") return false;
  const n = Number.parseInt(t, 10);
  return Number.isFinite(n) && n === minutes;
}

function formatPlannedPresetLabel(minutes: number): string {
  if (minutes <= 30) return `${minutes} mins`;
  const hours = minutes / 60;
  return `${hours} hour${hours === 1 ? "" : "s"}`;
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

function todayDateInputValue(d: Date = new Date()): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function startOfLocalDayUnix(d: Date = new Date()): number {
  const x = new Date(d);
  x.setHours(0, 0, 0, 0);
  return Math.floor(x.getTime() / 1000);
}

/** Week starts Monday (local). */
function startOfLocalWeekUnix(d: Date = new Date()): number {
  const x = new Date(d);
  const day = x.getDay();
  const diff = (day + 6) % 7;
  x.setDate(x.getDate() - diff);
  x.setHours(0, 0, 0, 0);
  return Math.floor(x.getTime() / 1000);
}

function exportRangeUnixBounds(
  preset: ExportRangePreset,
  customStart: string,
  customEnd: string,
): { sinceUnix: number; untilUnix: number } {
  const untilUnix = Math.floor(Date.now() / 1000);
  switch (preset) {
    case "today":
      return { sinceUnix: startOfLocalDayUnix(), untilUnix };
    case "this_week":
      return { sinceUnix: startOfLocalWeekUnix(), untilUnix };
    case "last_7":
      return { sinceUnix: untilUnix - 7 * 86_400, untilUnix };
    case "custom": {
      const start = customStart
        ? new Date(`${customStart}T00:00:00`)
        : new Date(0);
      const end = customEnd
        ? new Date(`${customEnd}T23:59:59`)
        : new Date(untilUnix * 1000);
      return {
        sinceUnix: Math.floor(start.getTime() / 1000),
        untilUnix: Math.floor(end.getTime() / 1000),
      };
    }
    default: {
      const _exhaustive: never = preset;
      return _exhaustive;
    }
  }
}

function formatExportRangeLabel(
  preset: ExportRangePreset,
  customStart: string,
  customEnd: string,
): string {
  const { sinceUnix, untilUnix } = exportRangeUnixBounds(
    preset,
    customStart,
    customEnd,
  );
  const opts: Intl.DateTimeFormatOptions = {
    dateStyle: "medium",
    timeStyle: "short",
  };
  return `${new Date(sinceUnix * 1000).toLocaleString(undefined, opts)} → ${new Date(untilUnix * 1000).toLocaleString(undefined, opts)}`;
}

function formatSegmentMinutes(m: number): string {
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  const r = m % 60;
  return r === 0 ? `${h} h` : `${h} h ${r} min`;
}

function formatHoursMinutesParts(totalMinutes: number): string {
  if (totalMinutes < 60) {
    return `${totalMinutes} minute${totalMinutes === 1 ? "" : "s"}`;
  }
  const hours = Math.floor(totalMinutes / 60);
  const mins = totalMinutes % 60;
  if (mins === 0) {
    return `${hours} hour${hours === 1 ? "" : "s"}`;
  }
  return `${hours} hour${hours === 1 ? "" : "s"} ${mins} minute${mins === 1 ? "" : "s"}`;
}

/** Green countdown or warm overdue line for timed "Currently on" tasks. */
function formatPlannedCountdown(
  plannedEndUnix: number,
  nowUnix: number,
): { text: string; overdue: boolean } {
  const diffSec = plannedEndUnix - nowUnix;
  const totalMinutes = Math.max(0, Math.ceil(Math.abs(diffSec) / 60));
  if (diffSec > 0) {
    return {
      text: `${formatHoursMinutesParts(totalMinutes)} left`,
      overdue: false,
    };
  }
  if (totalMinutes === 0) {
    return { text: "just overdue", overdue: true };
  }
  return {
    text: `${formatHoursMinutesParts(totalMinutes)} overdue`,
    overdue: true,
  };
}

function parseAdjustMinutes(input: string): number | null {
  const t = input.trim();
  if (t === "") return null;
  const n = Number.parseInt(t, 10);
  if (!Number.isFinite(n) || n < 1 || n > PING_MAX_MINUTES_CAP) return null;
  return n;
}

function formatElapsedSubtitle(
  startedAtUnix: number,
  nowUnix: number,
  plannedMinutes: number | null | undefined,
): string {
  const elapsedSec = Math.max(0, nowUnix - startedAtUnix);
  const totalMinutes = Math.floor(elapsedSec / 60);
  let base: string;
  if (totalMinutes < 60) {
    base = `for ${totalMinutes} minute${totalMinutes === 1 ? "" : "s"}`;
  } else {
    const hours = Math.floor(totalMinutes / 60);
    const mins = totalMinutes % 60;
    base = `for ${hours} hour${hours === 1 ? "" : "s"} ${mins} minute${mins === 1 ? "" : "s"}`;
  }
  if (plannedMinutes != null && plannedMinutes > 0) {
    return `${base} (${formatSegmentMinutes(plannedMinutes)} planned)`;
  }
  return base;
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

function formatGapWindow(adjustedEndUnix: number, gapMinutes: number): string {
  const from = new Date(adjustedEndUnix * 1000).toLocaleString(undefined, {
    dateStyle: "short",
    timeStyle: "short",
  });
  const to = new Date().toLocaleString(undefined, {
    dateStyle: "short",
    timeStyle: "short",
  });
  if (gapMinutes > 0) {
    return `From ${from} until now (${to}) — about ${formatSegmentMinutes(gapMinutes)} to log.`;
  }
  return `From ${from} until now (${to}).`;
}

/** Full Still label for screen readers (toast keeps `still_button_label` in Rust). */
function stillAriaLabel(body: string): string {
  const t = body.trim();
  return t ? `Still on: ${t}` : "Still — repeat last capture";
}

/** Quick-capture shell. All user-facing strings are English until i18n (see /I18N.md). */
export default function App() {
  const mainRef = useRef<HTMLElement>(null);
  const [scheduleAccordionVersion, setScheduleAccordionVersion] = useState(0);

  const labelId = useId();
  const quickPicksLegendId = useId();
  const plannedMinutesId = useId();
  const plannedPresetsLegendId = useId();
  const minId = useId();
  const maxId = useId();
  const overdueMinId = useId();
  const overdueMaxId = useId();
  const adjustMinutesId = useId();
  const manageExtendId = useId();
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
  const [randomPingEnabled, setRandomPingEnabled] = useState(true);
  const [overdueBoundsMin, setOverdueBoundsMin] = useState(15);
  const [overdueBoundsMax, setOverdueBoundsMax] = useState(30);
  const [overduePingEnabled, setOverduePingEnabled] = useState(true);
  const [adjustMinutes, setAdjustMinutes] = useState("15");
  const [manageExtendMinutes, setManageExtendMinutes] = useState("30");
  const [extendingManage, setExtendingManage] = useState(false);
  const [intervalError, setIntervalError] = useState<string | null>(null);
  const [overdueIntervalError, setOverdueIntervalError] = useState<string | null>(
    null,
  );
  const [applyingInterval, setApplyingInterval] = useState(false);
  const [applyingOverdueInterval, setApplyingOverdueInterval] = useState(false);
  const [dailyReminderEnabled, setDailyReminderEnabled] = useState(false);
  const [dailyReminders, setDailyReminders] = useState<DailyReminderRow[]>([]);
  const [dailyPresets, setDailyPresets] = useState<DailyReminderPreset[]>([]);
  const [dailyDraft, setDailyDraft] = useState<DailyReminderDraft | null>(null);
  const [dailyError, setDailyError] = useState<string | null>(null);
  const [dailySaving, setDailySaving] = useState(false);
  const [dailyTogglingEnabled, setDailyTogglingEnabled] = useState(false);
  const [sleepEnabled, setSleepEnabled] = useState(false);
  const [sleepStartHm, setSleepStartHm] = useState("22:00");
  const [sleepEndHm, setSleepEndHm] = useState("08:00");
  const [sleepError, setSleepError] = useState<string | null>(null);
  const [applyingSleep, setApplyingSleep] = useState(false);
  const [snoozing, setSnoozing] = useState(false);
  const [repeating, setRepeating] = useState(false);
  const [recentCaptures, setRecentCaptures] = useState<CaptureRow[]>([]);
  const [quickPicks, setQuickPicks] = useState<QuickPickRow[]>([]);
  const [mainTab, setMainTab] = useState<MainTab>("capture");
  const [digestPeriod, setDigestPeriod] = useState<DigestPeriod>("week");
  const [digest, setDigest] = useState<ActivityDigestRow[]>([]);
  const [digestCopied, setDigestCopied] = useState(false);
  const [exportPreset, setExportPreset] =
    useState<ExportRangePreset>("last_7");
  const [exportCustomStart, setExportCustomStart] = useState(() =>
    todayDateInputValue(),
  );
  const [exportCustomEnd, setExportCustomEnd] = useState(() =>
    todayDateInputValue(),
  );
  const [exportStatus, setExportStatus] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [adjustingMinus, setAdjustingMinus] = useState(false);
  const [adjustingPlus, setAdjustingPlus] = useState(false);
  const [markingDone, setMarkingDone] = useState(false);
  const [gapFillPrompt, setGapFillPrompt] = useState<ShortenGapPrompt | null>(
    null,
  );
  const [gapActivity, setGapActivity] = useState("");
  const [gapPlannedMinutes, setGapPlannedMinutes] = useState(
    String(DEFAULT_PLANNED_MINUTES),
  );
  const [gapSaving, setGapSaving] = useState(false);
  const gapActivityRef = useRef<HTMLTextAreaElement>(null);
  const [currentActivity, setCurrentActivity] =
    useState<CurrentActivityStatus | null>(null);
  const [nowUnix, setNowUnix] = useState(() =>
    Math.floor(Date.now() / 1000),
  );

  const latestCaptureBody = recentCaptures[0]?.body?.trim() ?? "";
  const canRepeatLast = latestCaptureBody.length > 0;
  const canAdjustDuration = recentCaptures.some(
    (r) => r.durationMinutes != null && r.durationMinutes > 0,
  );

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

  const refreshCurrentActivity = useCallback(async () => {
    try {
      const status = await invoke<CurrentActivityStatus>("get_current_activity");
      setCurrentActivity(status);
    } catch {
      setCurrentActivity(null);
    }
  }, []);

  const refreshCaptureLists = useCallback(async () => {
    await Promise.all([
      refreshRecentCaptures(),
      refreshQuickPicks(),
      refreshCurrentActivity(),
    ]);
  }, [refreshRecentCaptures, refreshQuickPicks, refreshCurrentActivity]);

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
        s.randomPingEnabled
          ? `Random interval: ${s.pingMinMinutes}–${s.pingMaxMinutes} min`
          : "Random ping: disabled",
      );
      setBoundsMin(s.pingMinMinutes);
      setBoundsMax(s.pingMaxMinutes);
      setRandomPingEnabled(s.randomPingEnabled);
      setOverdueBoundsMin(s.overduePingMinMinutes);
      setOverdueBoundsMax(s.overduePingMaxMinutes);
      setOverduePingEnabled(s.overduePingEnabled);
      setAwaitingFollowup(Boolean(s.awaitingFollowup));
      setPlannedCheckSubject(s.plannedCheckSubject ?? null);
    } catch {
      setSchedulerLine(null);
      setIntervalLine(null);
      setAwaitingFollowup(false);
      setPlannedCheckSubject(null);
    }
  }, []);

  const refreshDailyReminders = useCallback(async () => {
    try {
      const [settings, presets] = await Promise.all([
        invoke<DailyReminderSettings>("get_daily_reminder_settings"),
        invoke<DailyReminderPreset[]>("get_daily_reminder_presets"),
      ]);
      setDailyReminderEnabled(settings.enabled);
      setDailyReminders(settings.reminders);
      setDailyPresets(presets);
    } catch {
      setDailyReminders([]);
    }
  }, []);

  useEffect(() => {
    void refreshSchedulerStatus();
    void refreshCaptureLists();
  }, [refreshSchedulerStatus, refreshCaptureLists]);

  const refreshSleepHours = useCallback(async () => {
    try {
      const s = await invoke<SleepHoursSettings>("get_sleep_hours_settings");
      setSleepEnabled(s.enabled);
      setSleepStartHm(s.startHm);
      setSleepEndHm(s.endHm);
    } catch {
      setSleepEnabled(false);
      setSleepStartHm("22:00");
      setSleepEndHm("08:00");
    }
  }, []);

  useEffect(() => {
    if (mainTab !== "schedule") return;
    void refreshDailyReminders();
    void refreshSleepHours();
  }, [mainTab, refreshDailyReminders, refreshSleepHours]);

  useEffect(() => {
    if (mainTab !== "history") return;
    void refreshRecentCaptures();
    void refreshDigest();
  }, [mainTab, refreshRecentCaptures, refreshDigest]);

  useEffect(() => {
    if (mainTab !== "history") return;
    void refreshDigest();
  }, [digestPeriod, mainTab, refreshDigest]);

  useEffect(() => {
    if (mainTab !== "capture") return;
    const needsTick =
      currentActivity?.startedAtUnix != null ||
      currentActivity?.plannedEndAtUnix != null;
    if (!needsTick) return;
    const id = window.setInterval(() => {
      setNowUnix(Math.floor(Date.now() / 1000));
    }, 30_000);
    return () => window.clearInterval(id);
  }, [
    mainTab,
    currentActivity?.startedAtUnix,
    currentActivity?.plannedEndAtUnix,
  ]);

  useEffect(() => {
    if (mainTab === "capture") {
      setNowUnix(Math.floor(Date.now() / 1000));
    }
  }, [mainTab, currentActivity?.body, currentActivity?.startedAtUnix]);

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

  const openGapFillDialog = useCallback((prompt: ShortenGapPrompt) => {
    setError(null);
    setGapFillPrompt(prompt);
    setGapActivity(prompt.suggestedActivity);
    setGapPlannedMinutes(String(DEFAULT_PLANNED_MINUTES));
    setMainTab("capture");
    requestAnimationFrame(() => gapActivityRef.current?.focus());
  }, []);

  function closeGapFillDialog() {
    setGapFillPrompt(null);
    setGapActivity("");
    setGapSaving(false);
  }

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
        const offGapFill = await listen<ShortenGapPrompt>(
          "gap-fill-needed",
          (event) => {
            openGapFillDialog(event.payload);
          },
        );
        unlisten = () => {
          offPingDue();
          offScheduler();
          offGapFill();
        };
      } catch {
        // `npm run dev` without Tauri — no event bridge.
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refreshSchedulerStatus, refreshCaptureLists, openGapFillDialog]);

  async function onSnooze() {
    setSnoozing(true);
    try {
      await invoke<SchedulerStatus>("snooze_ping");
      void refreshSchedulerStatus();
    } finally {
      setSnoozing(false);
    }
  }

  async function onAdjustMinus() {
    setError(null);
    const minutes = parseAdjustMinutes(adjustMinutes);
    if (minutes == null) {
      setError(
        `Enter minutes between 1 and ${PING_MAX_MINUTES_CAP.toLocaleString()} to shorten.`,
      );
      return;
    }
    setAdjustingMinus(true);
    try {
      const prompt = await invoke<ShortenGapPrompt>("shorten_last_capture", {
        minutes,
      });
      openGapFillDialog(prompt);
      void refreshSchedulerStatus();
      void refreshCaptureLists();
    } catch (err) {
      setError(String(err));
    } finally {
      setAdjustingMinus(false);
    }
  }

  async function onAdjustPlus() {
    setError(null);
    const minutes = parseAdjustMinutes(adjustMinutes);
    if (minutes == null) {
      setError(
        `Enter minutes between 1 and ${PING_MAX_MINUTES_CAP.toLocaleString()} to extend.`,
      );
      return;
    }
    setAdjustingPlus(true);
    try {
      await invoke<SchedulerStatus>("extend_last_capture", { minutes });
      void refreshSchedulerStatus();
      void refreshCaptureLists();
    } catch (err) {
      setError(String(err));
    } finally {
      setAdjustingPlus(false);
    }
  }

  async function onManageContinue() {
    setError(null);
    const minutes = parseAdjustMinutes(manageExtendMinutes);
    if (minutes == null) {
      setError(
        `Enter minutes between 1 and ${PING_MAX_MINUTES_CAP.toLocaleString()} to continue.`,
      );
      return;
    }
    setExtendingManage(true);
    try {
      await invoke<SchedulerStatus>("extend_last_capture", { minutes });
      void refreshSchedulerStatus();
      void refreshCaptureLists();
    } catch (err) {
      setError(String(err));
    } finally {
      setExtendingManage(false);
    }
  }

  async function onDone() {
    setError(null);
    setMarkingDone(true);
    try {
      await invoke<SchedulerStatus>("mark_task_done");
      setText("");
      setPlannedMinutes(String(DEFAULT_PLANNED_MINUTES));
      void refreshSchedulerStatus();
      void refreshCaptureLists();
      requestAnimationFrame(() => textareaRef.current?.focus());
    } catch (err) {
      setError(String(err));
    } finally {
      setMarkingDone(false);
    }
  }

  async function onSubmitGapFill(e: React.FormEvent) {
    e.preventDefault();
    if (!gapFillPrompt) return;
    setError(null);
    const trimmed = gapActivity.trim();
    if (!trimmed) {
      setError("Choose what you were doing in that window.");
      return;
    }
    const rawMinutes = gapPlannedMinutes.trim();
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
    setGapSaving(true);
    try {
      await invoke<SchedulerStatus>("submit_gap_after_shorten", {
        input: {
          text: trimmed,
          gapMinutes: gapFillPrompt.gapMinutes,
          plannedDurationMinutes,
        },
      });
      closeGapFillDialog();
      void refreshSchedulerStatus();
      void refreshCaptureLists();
    } catch (err) {
      setError(String(err));
    } finally {
      setGapSaving(false);
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
        randomPingEnabled,
      });
      void refreshSchedulerStatus();
    } catch (err) {
      setIntervalError(String(err));
    } finally {
      setApplyingInterval(false);
    }
  }

  async function onToggleDailyReminderEnabled(enabled: boolean) {
    setDailyError(null);
    setDailyTogglingEnabled(true);
    try {
      const settings = await invoke<DailyReminderSettings>(
        "set_daily_reminder_enabled",
        { enabled },
      );
      setDailyReminderEnabled(settings.enabled);
      setDailyReminders(settings.reminders);
    } catch (err) {
      setDailyError(String(err));
    } finally {
      setDailyTogglingEnabled(false);
    }
  }

  function startEditDailyReminder(row: DailyReminderRow) {
    setDailyError(null);
    setDailyDraft({
      id: row.id,
      presetKey: row.presetKey ?? "custom",
      customLabel: row.presetKey ? "" : row.label,
      pillNote: row.pillNote ?? "",
      times: row.times.length > 0 ? [...row.times] : ["09:00"],
      enabled: row.enabled,
      burstCount: row.burstCount,
      burstIntervalMin: row.burstIntervalMin,
    });
    setScheduleAccordionVersion((v) => v + 1);
  }

  async function onSaveDailyReminder() {
    if (!dailyDraft) return;
    setDailyError(null);
    const label = draftLabelFromPreset(dailyDraft, dailyPresets);
    if (!label) {
      setDailyError("Choose a preset or enter a custom label.");
      return;
    }
    setDailySaving(true);
    try {
      const settings = await invoke<DailyReminderSettings>("save_daily_reminder", {
        input: {
          id: dailyDraft.id,
          label,
          times: dailyDraft.times.filter((t) => t.trim() !== ""),
          enabled: dailyDraft.enabled,
          burstCount: dailyDraft.burstCount,
          burstIntervalMin: dailyDraft.burstIntervalMin,
          presetKey:
            dailyDraft.presetKey === "custom" ? null : dailyDraft.presetKey,
          pillNote:
            dailyDraft.presetKey === "pills" && dailyDraft.pillNote.trim()
              ? dailyDraft.pillNote.trim()
              : null,
        },
      });
      setDailyReminderEnabled(settings.enabled);
      setDailyReminders(settings.reminders);
      setDailyDraft(null);
      setScheduleAccordionVersion((v) => v + 1);
    } catch (err) {
      setDailyError(String(err));
    } finally {
      setDailySaving(false);
    }
  }

  async function onDeleteDailyReminder(id: number) {
    setDailyError(null);
    try {
      const settings = await invoke<DailyReminderSettings>(
        "delete_daily_reminder",
        { id },
      );
      setDailyReminders(settings.reminders);
      if (dailyDraft?.id === id) setDailyDraft(null);
    } catch (err) {
      setDailyError(String(err));
    }
  }

  async function onApplySleepHours() {
    setSleepError(null);
    setApplyingSleep(true);
    try {
      const settings = await invoke<SleepHoursSettings>("save_sleep_hours_settings", {
        input: {
          enabled: sleepEnabled,
          startHm: sleepStartHm,
          endHm: sleepEndHm,
        },
      });
      setSleepEnabled(settings.enabled);
      setSleepStartHm(settings.startHm);
      setSleepEndHm(settings.endHm);
      await refreshSchedulerStatus();
    } catch (err) {
      setSleepError(String(err));
    } finally {
      setApplyingSleep(false);
    }
  }

  async function onApplyOverdueInterval() {
    setOverdueIntervalError(null);
    if (overdueBoundsMin < 1) {
      setOverdueIntervalError("Minimum must be at least 1 minute.");
      return;
    }
    if (overdueBoundsMax < overdueBoundsMin) {
      setOverdueIntervalError(
        "Maximum must be greater than or equal to minimum.",
      );
      return;
    }
    if (overdueBoundsMax > PING_MAX_MINUTES_CAP) {
      setOverdueIntervalError(
        `Maximum must be at most ${PING_MAX_MINUTES_CAP} minutes (one week).`,
      );
      return;
    }
    setApplyingOverdueInterval(true);
    try {
      await invoke<SchedulerStatus>("update_overdue_ping_interval", {
        overduePingMinMinutes: overdueBoundsMin,
        overduePingMaxMinutes: overdueBoundsMax,
        overduePingEnabled,
      });
      void refreshSchedulerStatus();
    } catch (err) {
      setOverdueIntervalError(String(err));
    } finally {
      setApplyingOverdueInterval(false);
    }
  }

  const runHistoryExport = useCallback(
    async (format: "csv" | "xlsx" | "pdf") => {
      setExporting(true);
      setExportStatus(null);
      const { sinceUnix, untilUnix } = exportRangeUnixBounds(
        exportPreset,
        exportCustomStart,
        exportCustomEnd,
      );
      try {
        if (format === "pdf") {
          await invoke("history_report_print", {
            sinceUnix,
            untilUnix,
          });
          setExportStatus(
            "Print dialog opened — choose “Save as PDF” or a printer.",
          );
          return;
        }
        const result = await invoke<ExportHistoryResult>("export_history_file", {
          input: { format, sinceUnix, untilUnix },
        });
        if (!result.saved) {
          setExportStatus("Export cancelled.");
          return;
        }
        setExportStatus(
          result.path ? `Saved to ${result.path}` : "Report saved.",
        );
      } catch (e) {
        setExportStatus(e instanceof Error ? e.message : String(e));
      } finally {
        setExporting(false);
      }
    },
    [exportPreset, exportCustomStart, exportCustomEnd],
  );

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
        input: {
          text: trimmed,
          plannedDurationMinutes,
        },
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

  const actionBusy =
    saving ||
    repeating ||
    adjustingMinus ||
    adjustingPlus ||
    markingDone ||
    extendingManage;

  const activeBody = currentActivity?.body?.trim() ?? "";
  const showCurrentActivity = activeBody.length > 0;
  const plannedEndUnix = currentActivity?.plannedEndAtUnix ?? null;
  const countdown =
    plannedEndUnix != null
      ? formatPlannedCountdown(plannedEndUnix, nowUnix)
      : null;
  const isOverdue = countdown?.overdue ?? false;
  const elapsedSubtitle =
    showCurrentActivity && currentActivity?.startedAtUnix != null
      ? formatElapsedSubtitle(
          currentActivity.startedAtUnix,
          nowUnix,
          currentActivity.durationMinutes,
        )
      : null;
  const showManage =
    showCurrentActivity &&
    currentActivity?.durationMinutes != null &&
    currentActivity.durationMinutes > 0;

  useFitWindowHeight(mainRef, [
    mainTab,
    scheduleAccordionVersion,
    gapFillPrompt != null,
    showCurrentActivity,
    showManage,
    awaitingFollowup,
    recentCaptures.length,
    digest.length,
    quickPicks.length,
  ]);

  return (
    <main
      ref={mainRef}
      className="mx-auto flex max-w-md flex-col gap-3 px-4 py-5"
    >
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
        <section
          className={`capture-status-card flex flex-col gap-0 p-3.5 ${
            isOverdue ? "border-action/35 bg-action/[0.04]" : ""
          }`}
        >
          <div className="flex items-start justify-between gap-3">
          <header className="min-w-0 flex-1 space-y-0.5">
            <p className="text-[0.65rem] font-medium uppercase tracking-wide text-ink/50">
              Currently on:
            </p>
            <h1 className="break-words text-lg font-semibold leading-snug tracking-tight text-ink">
              {showCurrentActivity ? activeBody : "Nothing"}
            </h1>
            {countdown ? (
              <p
                className={`text-sm font-medium tabular-nums ${
                  countdown.overdue ? "text-action" : "text-emerald-700"
                }`}
                aria-live="polite"
              >
                {countdown.text}
              </p>
            ) : null}
            {elapsedSubtitle ? (
              <p className="text-xs text-ink/65" aria-live="polite">
                {elapsedSubtitle}
              </p>
            ) : null}
          </header>
          <details className="group relative shrink-0">
            <summary
              className="flex h-7 w-7 cursor-pointer list-none items-center justify-center rounded-full border border-brand/25 bg-white/90 text-xs font-semibold text-ink/75 shadow-sm transition-colors hover:bg-brand/10 hover:text-ink focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand [&::-webkit-details-marker]:hidden"
              aria-label="Help"
            >
              ?
            </summary>
            <div
              role="tooltip"
              className="absolute right-0 top-full z-10 mt-2 w-[min(18rem,calc(100vw-2.5rem))] rounded-lg border border-brand/25 bg-white px-3 py-2.5 text-sm leading-snug text-ink/80 shadow-lg"
            >
              <p>
                What are you doing? Quick capture — honest answer. Ping
                timing lives on{" "}
                <button
                  type="button"
                  className="font-medium text-brand underline decoration-brand/35 underline-offset-2 hover:decoration-brand"
                  onClick={() => setMainTab("schedule")}
                >
                  Schedule
                </button>
                .
              </p>
            </div>
          </details>
          </div>

          <div
            className="mt-3 border-t border-brand/12 pt-3"
            role="toolbar"
            aria-label="Timing and repeat actions"
          >
            <div className="mb-2 flex items-end gap-1.5">
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <label
                  htmlFor={adjustMinutesId}
                  className="text-[0.65rem] font-medium text-ink/55"
                >
                  Adjust by (min)
                </label>
                <input
                  id={adjustMinutesId}
                  type="number"
                  min={1}
                  max={PING_MAX_MINUTES_CAP}
                  inputMode="numeric"
                  value={adjustMinutes}
                  onChange={(e) => setAdjustMinutes(e.target.value)}
                  disabled={actionBusy}
                  className="w-full rounded-md border border-brand/25 px-2 py-1.5 text-sm tabular-nums text-ink outline-none ring-brand/15 focus:border-brand focus:ring-2 disabled:opacity-60"
                />
              </div>
              <button
                type="button"
                onClick={() => void onAdjustMinus()}
                disabled={actionBusy || !canAdjustDuration}
                title="End your last logged segment earlier, then log what happened since."
                className="capture-primary-btn shrink-0 tabular-nums"
              >
                {adjustingMinus ? "…" : "−"}
              </button>
              <button
                type="button"
                onClick={() => void onAdjustPlus()}
                disabled={actionBusy || !canRepeatLast}
                title="Add minutes to your last segment and postpone the next ping."
                className="capture-primary-btn shrink-0 tabular-nums"
              >
                {adjustingPlus ? "…" : "+"}
              </button>
            </div>
            <div className="grid grid-cols-2 gap-1.5">
              <button
                type="button"
                onClick={() => void onDone()}
                disabled={actionBusy}
                title="Mark your current activity as finished and log elapsed time when you had no planned duration."
                className="capture-primary-btn"
              >
                {markingDone ? "…" : "Done"}
              </button>
              <button
                type="button"
                onClick={() => void onRepeatLast()}
                disabled={actionBusy || !canRepeatLast}
                title={
                  canRepeatLast
                    ? `Still on “${latestCaptureBody}” — log the same answer again.`
                    : "Save an answer once to enable Still."
                }
                aria-label={
                  canRepeatLast
                    ? stillAriaLabel(latestCaptureBody)
                    : "Still — save a capture first"
                }
                className="capture-primary-btn"
              >
                {repeating ? "…" : "Still"}
              </button>
            </div>
          </div>

          {showManage ? (
            <details
              className={`mt-3 border-t border-brand/12 pt-2 ${
                isOverdue ? "open:border-action/25" : ""
              }`}
              open={isOverdue || undefined}
            >
              <summary
                className={`cursor-pointer select-none text-sm font-medium ${
                  isOverdue ? "text-action" : "text-ink"
                }`}
              >
                Manage
              </summary>
              <div className="mt-2 space-y-2 rounded-lg border border-brand/15 bg-white/70 p-2.5">
                <p className="text-xs leading-snug text-ink/70">
                  {isOverdue
                    ? "Still on this? Extend your plan and push the next check."
                    : "Adjust how long you plan to keep at this."}
                </p>
                <div className="flex flex-wrap items-end gap-2">
                  <div className="min-w-[5.5rem] flex-1">
                    <label
                      htmlFor={manageExtendId}
                      className="text-[0.65rem] font-medium text-ink/55"
                    >
                      Continue (min)
                    </label>
                    <input
                      id={manageExtendId}
                      type="number"
                      min={1}
                      max={PING_MAX_MINUTES_CAP}
                      inputMode="numeric"
                      value={manageExtendMinutes}
                      onChange={(e) => setManageExtendMinutes(e.target.value)}
                      disabled={actionBusy}
                      className="mt-0.5 w-full rounded-md border border-brand/25 px-2 py-1.5 text-sm tabular-nums text-ink outline-none ring-brand/15 focus:border-brand focus:ring-2 disabled:opacity-60"
                    />
                  </div>
                  <button
                    type="button"
                    onClick={() => void onManageContinue()}
                    disabled={actionBusy}
                    className="capture-secondary-btn shrink-0"
                  >
                    {extendingManage ? "…" : "Extend plan"}
                  </button>
                </div>
              </div>
            </details>
          ) : null}
        </section>
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

          <details
            className="rounded-lg border border-brand/20 bg-white/80 px-3 py-2 shadow-sm open:pb-3"
            onToggle={() => setScheduleAccordionVersion((v) => v + 1)}
          >
            <summary className="flex cursor-pointer list-none select-none items-center gap-2 text-sm font-medium text-ink [&::-webkit-details-marker]:hidden">
              <span className="flex-1">Random ping</span>
              <button
                type="button"
                className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-brand/25 text-[0.65rem] font-semibold text-ink/60"
                aria-label="Random ping help"
                title='How often to ping when you chose "No idea" — no planned duration on the capture.'
                onClick={(e) => e.preventDefault()}
                onKeyDown={(e) => e.stopPropagation()}
              >
                ?
              </button>
            </summary>
            <div className="mt-3 flex flex-col gap-3">
              <label className="flex cursor-pointer items-center gap-2 text-sm text-ink/85">
                <input
                  type="checkbox"
                  checked={randomPingEnabled}
                  onChange={(e) => setRandomPingEnabled(e.target.checked)}
                  disabled={applyingInterval}
                  className="h-4 w-4 rounded border-brand/30 text-brand focus:ring-brand"
                />
                Enabled
              </label>
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
                    disabled={applyingInterval || !randomPingEnabled}
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
                    disabled={applyingInterval || !randomPingEnabled}
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
                {applyingInterval ? "Applying…" : "Apply random ping"}
              </button>
              {intervalError ? (
                <p className="text-sm font-medium text-red-600" role="status">
                  {intervalError}
                </p>
              ) : null}
            </div>
          </details>

          <details
            className="rounded-lg border border-brand/20 bg-white/80 px-3 py-2 shadow-sm open:pb-3"
            onToggle={() => setScheduleAccordionVersion((v) => v + 1)}
          >
            <summary className="flex cursor-pointer list-none select-none items-center gap-2 text-sm font-medium text-ink [&::-webkit-details-marker]:hidden">
              <span className="flex-1">Overdue ping</span>
              <button
                type="button"
                className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-brand/25 text-[0.65rem] font-semibold text-ink/60"
                aria-label="Overdue ping help"
                title="How often to ping after your planned finish time passed but you haven't marked the task done."
                onClick={(e) => e.preventDefault()}
                onKeyDown={(e) => e.stopPropagation()}
              >
                ?
              </button>
            </summary>
            <div className="mt-3 flex flex-col gap-3">
              <label className="flex cursor-pointer items-center gap-2 text-sm text-ink/85">
                <input
                  type="checkbox"
                  checked={overduePingEnabled}
                  onChange={(e) => setOverduePingEnabled(e.target.checked)}
                  disabled={applyingOverdueInterval}
                  className="h-4 w-4 rounded border-brand/30 text-brand focus:ring-brand"
                />
                Enabled
              </label>
              <div className="grid grid-cols-2 gap-3">
                <div className="flex flex-col gap-1">
                  <label
                    htmlFor={overdueMinId}
                    className="text-xs font-medium text-ink/80"
                  >
                    Min (minutes)
                  </label>
                  <input
                    id={overdueMinId}
                    type="number"
                    min={1}
                    max={PING_MAX_MINUTES_CAP}
                    value={overdueBoundsMin}
                    onChange={(e) =>
                      setOverdueBoundsMin(
                        Number.parseInt(e.target.value, 10) || 0,
                      )
                    }
                    disabled={applyingOverdueInterval || !overduePingEnabled}
                    className="rounded-md border border-brand/25 px-2 py-1.5 text-sm text-ink outline-none ring-brand/15 focus:border-brand focus:ring-2 disabled:opacity-60"
                  />
                </div>
                <div className="flex flex-col gap-1">
                  <label
                    htmlFor={overdueMaxId}
                    className="text-xs font-medium text-ink/80"
                  >
                    Max (minutes)
                  </label>
                  <input
                    id={overdueMaxId}
                    type="number"
                    min={1}
                    max={PING_MAX_MINUTES_CAP}
                    value={overdueBoundsMax}
                    onChange={(e) =>
                      setOverdueBoundsMax(
                        Number.parseInt(e.target.value, 10) || 0,
                      )
                    }
                    disabled={applyingOverdueInterval || !overduePingEnabled}
                    className="rounded-md border border-brand/25 px-2 py-1.5 text-sm text-ink outline-none ring-brand/15 focus:border-brand focus:ring-2 disabled:opacity-60"
                  />
                </div>
              </div>
              <button
                type="button"
                onClick={() => void onApplyOverdueInterval()}
                disabled={applyingOverdueInterval}
                className="self-start rounded-md border border-brand/30 bg-brand/10 px-3 py-1.5 text-sm font-medium text-ink transition-colors hover:bg-brand/15 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60"
              >
                {applyingOverdueInterval ? "Applying…" : "Apply overdue ping"}
              </button>
              {overdueIntervalError ? (
                <p className="text-sm font-medium text-red-600" role="status">
                  {overdueIntervalError}
                </p>
              ) : null}
            </div>
          </details>

          <details
            className="rounded-lg border border-brand/20 bg-white/80 px-3 py-2 shadow-sm open:pb-3"
            onToggle={() => setScheduleAccordionVersion((v) => v + 1)}
          >
            <summary className="flex cursor-pointer list-none select-none items-center gap-2 text-sm font-medium text-ink [&::-webkit-details-marker]:hidden">
              <span className="flex-1">Sleeping hours</span>
              <button
                type="button"
                className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-brand/25 text-[0.65rem] font-semibold text-ink/60"
                aria-label="Sleeping hours help"
                title="Quiet hours: no activity pings (random, overdue, planned) and no daily reminders. Pings resume after wake time."
                onClick={(e) => e.preventDefault()}
                onKeyDown={(e) => e.stopPropagation()}
              >
                ?
              </button>
            </summary>
            <div className="mt-3 flex flex-col gap-3">
              <label className="flex cursor-pointer items-center gap-2 text-sm text-ink/85">
                <input
                  type="checkbox"
                  checked={sleepEnabled}
                  onChange={(e) => setSleepEnabled(e.target.checked)}
                  disabled={applyingSleep}
                  className="h-4 w-4 rounded border-brand/30 text-brand focus:ring-brand"
                />
                Enabled
              </label>
              <div className="grid grid-cols-2 gap-3">
                <div className="flex flex-col gap-1">
                  <span className="text-xs font-medium text-ink/80">
                    Sleep from
                  </span>
                  <TimePickerField
                    id="sleep-start"
                    value={sleepStartHm}
                    onChange={setSleepStartHm}
                    disabled={applyingSleep || !sleepEnabled}
                    presets={SLEEP_START_PRESETS}
                    aria-label="Sleep start"
                  />
                </div>
                <div className="flex flex-col gap-1">
                  <span className="text-xs font-medium text-ink/80">
                    Wake at
                  </span>
                  <TimePickerField
                    id="sleep-end"
                    value={sleepEndHm}
                    onChange={setSleepEndHm}
                    disabled={applyingSleep || !sleepEnabled}
                    presets={SLEEP_END_PRESETS}
                    aria-label="Wake time"
                  />
                </div>
              </div>
              <p className="text-[0.65rem] leading-snug text-ink/45">
                Uses your system timezone. Default 22:00–08:00. Scheduled pings
                move to wake time; nothing fires while you sleep.
              </p>
              <button
                type="button"
                onClick={() => void onApplySleepHours()}
                disabled={applyingSleep}
                className="self-start cursor-pointer rounded-md border border-brand/30 bg-brand/10 px-3 py-1.5 text-sm font-medium text-ink transition-colors hover:bg-brand/15 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60"
              >
                {applyingSleep ? "Applying…" : "Apply sleeping hours"}
              </button>
              {sleepError ? (
                <p className="text-sm font-medium text-red-600" role="status">
                  {sleepError}
                </p>
              ) : null}
            </div>
          </details>

          <details
            className="rounded-lg border border-brand/20 bg-white/80 px-3 py-2 shadow-sm open:pb-3"
            onToggle={() => setScheduleAccordionVersion((v) => v + 1)}
          >
            <summary className="flex cursor-pointer list-none select-none items-center gap-2 text-sm font-medium text-ink [&::-webkit-details-marker]:hidden">
              <span className="flex-1">Daily reminder ping</span>
              <button
                type="button"
                className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-brand/25 text-[0.65rem] font-semibold text-ink/60"
                aria-label="Daily reminder help"
                title="Gentle self-care nudges (water, food, meds, breaks). Separate from activity pings. Off by default."
                onClick={(e) => e.preventDefault()}
                onKeyDown={(e) => e.stopPropagation()}
              >
                ?
              </button>
            </summary>
            <div className="mt-3 flex flex-col gap-3">
              <label className="flex cursor-pointer items-center gap-2 text-sm text-ink/85">
                <input
                  type="checkbox"
                  checked={dailyReminderEnabled}
                  onChange={(e) =>
                    void onToggleDailyReminderEnabled(e.target.checked)
                  }
                  disabled={dailyTogglingEnabled}
                  className="h-4 w-4 rounded border-brand/30 text-brand focus:ring-brand"
                />
                Enabled
              </label>
              <p className="text-[0.65rem] leading-snug text-ink/45">
                For hyperfocus days — basics like water, food, or medication.
                Tap Done on a notification to stop further nudges for that
                reminder only.
              </p>

              {dailyReminders.length > 0 ? (
                <ul className="flex flex-col gap-2" aria-label="Daily reminders">
                  {dailyReminders.map((r) => (
                    <li
                      key={r.id}
                      className="flex flex-wrap items-center gap-2 rounded-md border border-brand/15 bg-brand/[0.03] px-2 py-1.5 text-xs"
                    >
                      <span className="font-medium text-ink/90">
                        {r.label}
                        {!r.enabled ? (
                          <span className="ml-1 font-normal text-ink/45">
                            (paused)
                          </span>
                        ) : null}
                      </span>
                      <span className="text-ink/50">
                        {r.times.join(", ")}
                        {r.burstCount > 1
                          ? ` · nudge ${r.burstCount}×/${r.burstIntervalMin}m`
                          : null}
                      </span>
                      <span className="ml-auto flex gap-1">
                        <button
                          type="button"
                          onClick={() => startEditDailyReminder(r)}
                          className="cursor-pointer rounded border border-brand/25 px-2 py-0.5 text-ink/80 hover:bg-brand/10"
                        >
                          Edit
                        </button>
                        <button
                          type="button"
                          onClick={() => void onDeleteDailyReminder(r.id)}
                          className="cursor-pointer rounded border border-red-200 px-2 py-0.5 text-red-700 hover:bg-red-50"
                        >
                          Delete
                        </button>
                      </span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-xs text-ink/50">
                  No reminders yet. Add one below.
                </p>
              )}

              {dailyDraft ? (
                <div className="flex flex-col gap-3 rounded-md border border-brand/20 bg-white/90 p-3">
                  <p className="text-xs font-medium text-ink/70">
                    {dailyDraft.id == null ? "New reminder" : "Edit reminder"}
                  </p>
                  <div className="flex flex-wrap gap-1.5" role="group" aria-label="Preset">
                    {dailyPresets.map((p) => (
                      <button
                        key={p.key}
                        type="button"
                        onClick={() =>
                          setDailyDraft((d) =>
                            d ? { ...d, presetKey: p.key } : d,
                          )
                        }
                        className={`cursor-pointer rounded-full border px-2 py-0.5 text-[0.7rem] transition-colors ${
                          dailyDraft.presetKey === p.key
                            ? "border-brand bg-brand/15 font-medium text-ink"
                            : "border-brand/25 text-ink/70 hover:bg-brand/10"
                        }`}
                      >
                        {p.label}
                      </button>
                    ))}
                    <button
                      type="button"
                      onClick={() =>
                        setDailyDraft((d) =>
                          d ? { ...d, presetKey: "custom" } : d,
                        )
                      }
                      className={`cursor-pointer rounded-full border px-2 py-0.5 text-[0.7rem] transition-colors ${
                        dailyDraft.presetKey === "custom"
                          ? "border-brand bg-brand/15 font-medium text-ink"
                          : "border-brand/25 text-ink/70 hover:bg-brand/10"
                      }`}
                    >
                      Custom
                    </button>
                  </div>
                  {dailyDraft.presetKey === "custom" ? (
                    <input
                      type="text"
                      value={dailyDraft.customLabel}
                      onChange={(e) =>
                        setDailyDraft((d) =>
                          d ? { ...d, customLabel: e.target.value } : d,
                        )
                      }
                      placeholder="Reminder text"
                      className="rounded-md border border-brand/25 px-2 py-1.5 text-sm text-ink outline-none focus:border-brand focus:ring-2 focus:ring-brand/15"
                    />
                  ) : null}
                  {dailyDraft.presetKey === "pills" ? (
                    <input
                      type="text"
                      value={dailyDraft.pillNote}
                      onChange={(e) =>
                        setDailyDraft((d) =>
                          d ? { ...d, pillNote: e.target.value } : d,
                        )
                      }
                      placeholder="Medication name (optional)"
                      className="rounded-md border border-brand/25 px-2 py-1.5 text-sm text-ink outline-none focus:border-brand focus:ring-2 focus:ring-brand/15"
                    />
                  ) : null}
                  <div className="flex flex-col gap-2">
                    <span className="text-xs font-medium text-ink/80">
                      Times (local)
                    </span>
                    {dailyDraft.times.map((t, i) => (
                      <div
                        key={i}
                        className="flex flex-col gap-2 sm:flex-row sm:items-start sm:gap-2"
                      >
                        <TimePickerField
                          id={`daily-time-${i}`}
                          value={t}
                          onChange={(next) =>
                            setDailyDraft((d) => {
                              if (!d) return d;
                              const times = [...d.times];
                              times[i] = next;
                              return { ...d, times };
                            })
                          }
                          presets={DAILY_REMINDER_TIME_PRESETS}
                          aria-label={`Reminder time ${i + 1}`}
                        />
                        {dailyDraft.times.length > 1 ? (
                          <button
                            type="button"
                            onClick={() =>
                              setDailyDraft((d) => {
                                if (!d) return d;
                                return {
                                  ...d,
                                  times: d.times.filter((_, j) => j !== i),
                                };
                              })
                            }
                            className="cursor-pointer text-xs text-ink/50 hover:text-red-600"
                          >
                            Remove
                          </button>
                        ) : null}
                      </div>
                    ))}
                    <button
                      type="button"
                      onClick={() =>
                        setDailyDraft((d) =>
                          d ? { ...d, times: [...d.times, "12:00"] } : d,
                        )
                      }
                      className="self-start cursor-pointer text-xs font-medium text-brand hover:underline"
                    >
                      + Add time
                    </button>
                  </div>
                  <fieldset className="flex flex-col gap-2 border-0 p-0">
                    <legend className="text-xs font-medium text-ink/80">
                      Nudge until done
                    </legend>
                    <p className="text-[0.65rem] leading-snug text-ink/45">
                      Repeat this reminder a few times until you tap Done on the
                      notification.
                    </p>
                    <div className="grid grid-cols-2 gap-3">
                      <div className="flex flex-col gap-1">
                        <label className="text-xs text-ink/70">Count</label>
                        <input
                          type="number"
                          min={1}
                          max={10}
                          value={dailyDraft.burstCount}
                          onChange={(e) =>
                            setDailyDraft((d) =>
                              d
                                ? {
                                    ...d,
                                    burstCount:
                                      Number.parseInt(e.target.value, 10) ||
                                      1,
                                  }
                                : d,
                            )
                          }
                          className="rounded-md border border-brand/25 px-2 py-1.5 text-sm"
                        />
                      </div>
                      <div className="flex flex-col gap-1">
                        <label className="text-xs text-ink/70">
                          Every (min)
                        </label>
                        <input
                          type="number"
                          min={1}
                          max={60}
                          value={dailyDraft.burstIntervalMin}
                          onChange={(e) =>
                            setDailyDraft((d) =>
                              d
                                ? {
                                    ...d,
                                    burstIntervalMin:
                                      Number.parseInt(e.target.value, 10) ||
                                      5,
                                  }
                                : d,
                            )
                          }
                          className="rounded-md border border-brand/25 px-2 py-1.5 text-sm"
                        />
                      </div>
                    </div>
                  </fieldset>
                  <label className="flex cursor-pointer items-center gap-2 text-sm text-ink/85">
                    <input
                      type="checkbox"
                      checked={dailyDraft.enabled}
                      onChange={(e) =>
                        setDailyDraft((d) =>
                          d ? { ...d, enabled: e.target.checked } : d,
                        )
                      }
                      className="h-4 w-4 rounded border-brand/30 text-brand"
                    />
                    Reminder active
                  </label>
                  <div className="flex flex-wrap gap-2">
                    <button
                      type="button"
                      disabled={dailySaving}
                      onClick={() => void onSaveDailyReminder()}
                      className="cursor-pointer rounded-md border border-brand/30 bg-brand/10 px-3 py-1.5 text-sm font-medium text-ink hover:bg-brand/15 disabled:opacity-60"
                    >
                      {dailySaving ? "Saving…" : "Save reminder"}
                    </button>
                    <button
                      type="button"
                      onClick={() => setDailyDraft(null)}
                      className="cursor-pointer rounded-md border border-brand/20 px-3 py-1.5 text-sm text-ink/70 hover:bg-brand/5"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : (
                <button
                  type="button"
                  disabled={!dailyReminderEnabled}
                  onClick={() => {
                    setDailyDraft(emptyDailyDraft());
                    setScheduleAccordionVersion((v) => v + 1);
                  }}
                  className="self-start cursor-pointer rounded-md border border-dashed border-brand/35 px-3 py-1.5 text-sm text-ink/80 hover:bg-brand/5 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  + Add reminder
                </button>
              )}
              {dailyError ? (
                <p className="text-sm font-medium text-red-600" role="status">
                  {dailyError}
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

          <div className="flex flex-col gap-2 rounded-lg border border-brand/20 bg-white/90 p-3">
            <p className="text-xs font-medium uppercase tracking-wide text-ink/55">
              Export report
            </p>
            <p className="text-xs leading-snug text-ink/60">
              Overall totals plus a chronological timeline for the selected
              range. PDF opens the system print dialog (Save as PDF).
            </p>
            <div
              className="flex flex-wrap gap-1.5"
              role="group"
              aria-label="Export date range"
            >
              {(
                [
                  ["today", "Today"],
                  ["this_week", "This week"],
                  ["last_7", "Last 7 days"],
                  ["custom", "Custom"],
                ] as const
              ).map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  disabled={exporting}
                  aria-pressed={exportPreset === id}
                  onClick={() => setExportPreset(id)}
                  className={`cursor-pointer rounded-full border px-2.5 py-1 text-xs font-medium transition-colors duration-interaction focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-50 ${
                    exportPreset === id
                      ? "border-brand bg-brand/15 text-ink"
                      : "border-brand/25 bg-white text-ink/80 hover:border-brand/35"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
            {exportPreset === "custom" ? (
              <div className="flex flex-wrap items-end gap-2">
                <label className="flex flex-col gap-0.5 text-xs text-ink/70">
                  From
                  <input
                    type="date"
                    value={exportCustomStart}
                    disabled={exporting}
                    onChange={(e) => setExportCustomStart(e.target.value)}
                    className="cursor-pointer rounded-md border border-brand/25 bg-white px-2 py-1 text-sm text-ink"
                  />
                </label>
                <label className="flex flex-col gap-0.5 text-xs text-ink/70">
                  To
                  <input
                    type="date"
                    value={exportCustomEnd}
                    disabled={exporting}
                    onChange={(e) => setExportCustomEnd(e.target.value)}
                    className="cursor-pointer rounded-md border border-brand/25 bg-white px-2 py-1 text-sm text-ink"
                  />
                </label>
              </div>
            ) : null}
            <p className="text-[0.65rem] text-ink/50 tabular-nums">
              {formatExportRangeLabel(
                exportPreset,
                exportCustomStart,
                exportCustomEnd,
              )}
            </p>
            <div
              className="flex flex-wrap gap-2"
              role="group"
              aria-label="Export format"
            >
              {(
                [
                  ["csv", "CSV"],
                  ["xlsx", "XLSX"],
                  ["pdf", "PDF"],
                ] as const
              ).map(([fmt, label]) => (
                <button
                  key={fmt}
                  type="button"
                  disabled={exporting}
                  onClick={() => void runHistoryExport(fmt)}
                  className="cursor-pointer rounded-md border border-brand/30 bg-white px-3 py-1.5 text-xs font-medium text-ink/90 transition-colors duration-interaction hover:bg-brand/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {exporting ? "Exporting…" : label}
                </button>
              ))}
            </div>
            {exportStatus ? (
              <p className="text-xs text-ink/65" role="status">
                {exportStatus}
              </p>
            ) : null}
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
      <form onSubmit={onSubmit} className="flex flex-col gap-2.5">
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
              rows={4}
              disabled={saving || repeating}
              placeholder="Honest answer…"
              className="min-h-[6.5rem] w-full resize-y rounded-none border-0 bg-transparent px-3 py-2.5 text-sm text-ink outline-none placeholder:text-ink/40 focus:ring-0 disabled:cursor-not-allowed"
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
              ? 'Use preset chips in the input card, or custom minutes here. “No idea” / blank keeps random next ping (if enabled on Schedule).'
              : `Use preset chips in the input card or type custom 1–${PING_MAX_MINUTES_CAP.toLocaleString()} mins. “No idea” / blank uses random ping when enabled on Schedule.`}
          </p>
          <div className="overflow-hidden rounded-md border border-brand/25 bg-white">
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
              className="w-full border-0 bg-transparent px-2.5 py-2 text-sm text-ink outline-none ring-brand/15 transition-colors duration-200 placeholder:text-ink/38 focus:ring-2 disabled:cursor-not-allowed disabled:opacity-60"
            />
            <div
              className="border-t border-brand/15 bg-brand/[0.03] px-2 pb-2 pt-1.5"
              role="group"
              aria-labelledby={plannedPresetsLegendId}
            >
              <div className="flex flex-wrap gap-1.5">
                <button
                  type="button"
                  disabled={saving || repeating}
                  title="No fixed duration — next ping uses random interval"
                  aria-label="No idea — clear planned minutes"
                  aria-pressed={isUnsetPlannedMinutes(plannedMinutes)}
                  onClick={() => setPlannedMinutes("")}
                  className={`cursor-pointer rounded-full border px-2 py-0.5 text-[0.7rem] font-medium transition-colors duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60 ${
                    isUnsetPlannedMinutes(plannedMinutes)
                      ? "border-brand bg-brand/15 text-ink"
                      : "border-brand/20 bg-white/90 text-ink/90 hover:border-brand/35 hover:bg-brand/8"
                  }`}
                >
                  No idea
                </button>
                {PLANNED_DURATION_PRESETS.map((m) => {
                  const active = minutesMatchPreset(plannedMinutes, m);
                  const label = formatPlannedPresetLabel(m);
                  return (
                    <button
                      key={m}
                      type="button"
                      disabled={saving || repeating}
                      title={`${label}`}
                      aria-label={`Set planned duration to ${label}`}
                      aria-pressed={active}
                      onClick={() => setPlannedMinutes(String(m))}
                      className={`cursor-pointer rounded-full border px-2 py-0.5 text-[0.7rem] font-medium tabular-nums transition-colors duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60 ${
                        active
                          ? "border-brand bg-brand/15 text-ink"
                          : "border-brand/20 bg-white/90 text-ink/90 hover:border-brand/35 hover:bg-brand/8"
                      }`}
                    >
                      {label}
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        </div>

        <div>
          <button
            type="submit"
            disabled={actionBusy}
            className="w-full cursor-pointer rounded-lg bg-action px-4 py-2.5 text-sm font-semibold text-white shadow-sm transition-colors duration-interaction hover:bg-action-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-action motion-safe:active:scale-[0.99] disabled:cursor-not-allowed disabled:opacity-60"
          >
            {saving ? "Saving…" : "Save"}
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

      {gapFillPrompt ? (
        <div
          className="fixed inset-0 z-50 flex items-end justify-center bg-ink/25 p-4 sm:items-center"
          role="presentation"
          onClick={(e) => {
            if (e.target === e.currentTarget && !gapSaving) closeGapFillDialog();
          }}
        >
          <form
            role="dialog"
            aria-modal="true"
            aria-labelledby="gap-fill-title"
            onSubmit={(e) => void onSubmitGapFill(e)}
            className="flex max-h-[min(90vh,32rem)] w-full max-w-md flex-col gap-3 overflow-y-auto rounded-xl border border-brand/25 bg-white px-4 py-4 shadow-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <header className="space-y-1">
              <h2
                id="gap-fill-title"
                className="text-base font-semibold tracking-tight text-ink"
              >
                Log the time since you wrapped up early
              </h2>
              <p className="text-xs leading-snug text-ink/70">
                {formatGapWindow(
                  gapFillPrompt.adjustedEndUnix,
                  gapFillPrompt.gapMinutes,
                )}
              </p>
            </header>

            <div className="flex flex-col gap-2">
              <label
                htmlFor="gap-activity"
                className="text-sm font-medium text-ink"
              >
                What were you doing?
              </label>
              <div className="flex flex-col overflow-hidden rounded-lg border border-brand/25 bg-white shadow-sm ring-brand/20 focus-within:border-brand focus-within:ring-[3px]">
                <textarea
                  ref={gapActivityRef}
                  id="gap-activity"
                  value={gapActivity}
                  onChange={(e) => setGapActivity(e.target.value)}
                  rows={3}
                  disabled={gapSaving}
                  placeholder="Honest answer for this window…"
                  className="w-full resize-y rounded-none border-0 bg-transparent px-3 py-2 text-sm text-ink outline-none placeholder:text-ink/40 disabled:opacity-60"
                />
                {quickPicks.length > 0 ? (
                  <div className="border-t border-brand/15 bg-brand/[0.04] px-2 pb-2 pt-1.5">
                    <p className="mb-1 text-[0.65rem] font-medium uppercase tracking-wide text-ink/45">
                      Common answers
                    </p>
                    <div className="flex flex-wrap gap-1.5">
                      {quickPicks.map((pick) => (
                        <button
                          key={`gap-${pick.body}`}
                          type="button"
                          disabled={gapSaving}
                          title={pick.body}
                          aria-label={`Use quick answer: ${pick.body}`}
                          onClick={() => {
                            setGapActivity(pick.body);
                            requestAnimationFrame(() =>
                              gapActivityRef.current?.focus(),
                            );
                          }}
                          className="inline-flex max-w-full min-w-0 cursor-pointer items-center rounded-full border border-brand/20 bg-white/90 px-2 py-0.5 text-left text-[0.7rem] font-medium text-ink/90 shadow-sm transition-colors hover:border-brand/35 hover:bg-brand/8 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:opacity-60"
                        >
                          <span className="truncate">{pick.body}</span>
                        </button>
                      ))}
                    </div>
                  </div>
                ) : null}
              </div>
            </div>

            <div className="flex flex-col gap-2">
              <label
                htmlFor="gap-planned-minutes"
                className="text-xs font-medium text-ink/75"
              >
                Plan to keep at this for about (minutes)
              </label>
              <input
                id="gap-planned-minutes"
                type="number"
                min={1}
                max={PING_MAX_MINUTES_CAP}
                inputMode="numeric"
                value={gapPlannedMinutes}
                onChange={(e) => setGapPlannedMinutes(e.target.value)}
                disabled={gapSaving}
                placeholder="Blank → random next ping"
                className="rounded-md border border-brand/25 px-2.5 py-2 text-sm text-ink outline-none ring-brand/15 focus:border-brand focus:ring-2 disabled:opacity-60"
              />
              <div className="flex flex-wrap gap-1.5" role="group" aria-label="Planned duration presets">
                <button
                  type="button"
                  disabled={gapSaving}
                  aria-pressed={isUnsetPlannedMinutes(gapPlannedMinutes)}
                  onClick={() => setGapPlannedMinutes("")}
                  className={`cursor-pointer rounded-full border px-2 py-0.5 text-[0.7rem] font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:opacity-60 ${
                    isUnsetPlannedMinutes(gapPlannedMinutes)
                      ? "border-brand bg-brand/15 text-ink"
                      : "border-brand/20 bg-white/90 text-ink/90 hover:border-brand/35"
                  }`}
                >
                  No idea
                </button>
                {PLANNED_DURATION_PRESETS.map((m) => {
                  const active = minutesMatchPreset(gapPlannedMinutes, m);
                  const label = formatPlannedPresetLabel(m);
                  return (
                    <button
                      key={`gap-preset-${m}`}
                      type="button"
                      disabled={gapSaving}
                      aria-pressed={active}
                      onClick={() => setGapPlannedMinutes(String(m))}
                      className={`cursor-pointer rounded-full border px-2 py-0.5 text-[0.7rem] font-medium tabular-nums transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:opacity-60 ${
                        active
                          ? "border-brand bg-brand/15 text-ink"
                          : "border-brand/20 bg-white/90 text-ink/90 hover:border-brand/35"
                      }`}
                    >
                      {label}
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="flex flex-wrap gap-2 pt-1">
              <button
                type="submit"
                disabled={gapSaving}
                className="cursor-pointer rounded-lg bg-action px-4 py-2 text-sm font-semibold text-white shadow-sm transition-colors hover:bg-action-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-action disabled:opacity-60"
              >
                {gapSaving ? "Saving…" : "Save gap & plan"}
              </button>
              <button
                type="button"
                disabled={gapSaving}
                onClick={closeGapFillDialog}
                className="cursor-pointer rounded-lg border border-brand/30 bg-white px-4 py-2 text-sm font-medium text-ink/85 transition-colors hover:bg-brand/10 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:opacity-60"
              >
                Skip for now
              </button>
            </div>
          </form>
        </div>
      ) : null}
    </main>
  );
}
