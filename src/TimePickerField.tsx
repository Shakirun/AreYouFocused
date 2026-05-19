const HOURS = Array.from({ length: 24 }, (_, i) =>
  String(i).padStart(2, "0"),
);
const MINUTES = Array.from({ length: 60 }, (_, i) =>
  String(i).padStart(2, "0"),
);

const selectClass =
  "cursor-pointer rounded-md border border-brand/25 bg-white px-2 py-1.5 text-sm tabular-nums text-ink outline-none focus:border-brand focus:ring-2 focus:ring-brand/15 disabled:cursor-not-allowed disabled:opacity-60";

export const DAILY_REMINDER_TIME_PRESETS = [
  "07:00",
  "08:00",
  "09:00",
  "12:00",
  "13:00",
  "18:00",
  "21:00",
] as const;

export const SLEEP_START_PRESETS = ["21:00", "22:00", "23:00"] as const;
export const SLEEP_END_PRESETS = ["06:00", "07:00", "08:00", "09:00"] as const;

type TimePickerFieldProps = {
  id?: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  presets?: readonly string[];
  "aria-label"?: string;
};

function parseHm(value: string): [string, string] {
  const [h, m] = value.split(":");
  const hour = /^\d{1,2}$/.test(h ?? "")
    ? String(Number(h)).padStart(2, "0")
    : "09";
  const minute = /^\d{1,2}$/.test(m ?? "")
    ? String(Number(m)).padStart(2, "0")
    : "00";
  return [hour, minute];
}

export function TimePickerField({
  id,
  value,
  onChange,
  disabled,
  presets,
  "aria-label": ariaLabel,
}: TimePickerFieldProps) {
  const [hour, minute] = parseHm(value);
  const hourId = id ? `${id}-hour` : undefined;
  const minuteId = id ? `${id}-minute` : undefined;

  return (
    <div className="flex flex-col gap-2">
      {presets && presets.length > 0 ? (
        <div
          role="group"
          aria-label={ariaLabel ? `${ariaLabel} presets` : "Time presets"}
          className="flex flex-wrap gap-1"
        >
          {presets.map((t) => {
            const active = t === `${hour}:${minute}`;
            return (
              <button
                key={t}
                type="button"
                disabled={disabled}
                aria-pressed={active}
                onClick={() => onChange(t)}
                className={`cursor-pointer rounded-full border px-2 py-0.5 text-xs font-medium tabular-nums transition-colors duration-interaction focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-50 ${
                  active
                    ? "border-brand bg-brand/15 text-ink"
                    : "border-brand/25 bg-white/95 text-ink/75 hover:border-brand/40 hover:bg-brand/5"
                }`}
              >
                {t}
              </button>
            );
          })}
        </div>
      ) : null}
      <div className="flex items-center gap-1.5">
        <label className="sr-only" htmlFor={hourId}>
          Hour
        </label>
        <select
          id={hourId}
          value={hour}
          disabled={disabled}
          aria-label={ariaLabel ? `${ariaLabel} hour` : "Hour"}
          onChange={(e) => onChange(`${e.target.value}:${minute}`)}
          className={selectClass}
        >
          {HOURS.map((h) => (
            <option key={h} value={h}>
              {h}
            </option>
          ))}
        </select>
        <span className="text-sm font-medium text-ink/50" aria-hidden>
          :
        </span>
        <label className="sr-only" htmlFor={minuteId}>
          Minute
        </label>
        <select
          id={minuteId}
          value={minute}
          disabled={disabled}
          aria-label={ariaLabel ? `${ariaLabel} minute` : "Minute"}
          onChange={(e) => onChange(`${hour}:${e.target.value}`)}
          className={selectClass}
        >
          {MINUTES.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
