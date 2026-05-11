import { invoke } from "@tauri-apps/api/core";
import { useEffect, useId, useState } from "react";

/** Quick-capture shell. All user-facing strings are English until i18n (see /I18N.md). */
export default function App() {
  const labelId = useId();
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [lastPingAt, setLastPingAt] = useState<string | null>(null);

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
        });
      } catch {
        // `npm run dev` without Tauri — no event bridge.
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

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
      </header>

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
    </main>
  );
}
