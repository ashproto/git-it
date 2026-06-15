// Date helpers. Mirror the semantics of the Python helpers in
// legacy/git_it.py (parse_iso, to_local_str, epoch_tz_bytes).

/** User-selectable display options for rendering commit timestamps. */
export type DateFormatPrefs = {
  hour12: boolean;
  weekday: boolean;
  monthName: boolean;
};

// Fixed, locale-independent abbreviations — keeps the table aligned and avoids
// Intl locale surprises across machines.
const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]; // getDay() 0..6
const MONTHS = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun",
  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
]; // getMonth() 0..11

export function parseISO(iso: string): Date | null {
  if (!iso) return null;
  const cleaned = iso.trim().replace("Z", "+00:00");
  const d = new Date(cleaned);
  return Number.isNaN(d.getTime()) ? null : d;
}

/**
 * Render a commit timestamp honoring the user's display preferences. Display-only:
 * the git rewrite path never goes through here (it uses toEpochTz/formatTzOffset).
 * With every pref false this returns the default `YYYY-MM-DD HH:mm:ss ±HHMM`.
 */
export function formatCommitDate(d: Date, p: DateFormatPrefs): string {
  const pad = (n: number, w = 2) => String(n).padStart(w, "0");
  const parts: string[] = [];

  if (p.weekday) parts.push(WEEKDAYS[d.getDay()]);

  if (p.monthName) {
    // "MMM D, YYYY" — day not zero-padded, matching the "Jan 5, 2026" convention.
    parts.push(`${MONTHS[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`);
  } else {
    parts.push(`${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`);
  }

  if (p.hour12) {
    const h24 = d.getHours();
    const h12 = ((h24 + 11) % 12) + 1; // 0->12, 13->1, 23->11
    const suffix = h24 < 12 ? "AM" : "PM";
    parts.push(`${h12}:${pad(d.getMinutes())}:${pad(d.getSeconds())} ${suffix}`);
  } else {
    parts.push(`${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`);
  }

  parts.push(formatTzOffset(d));
  return parts.join(" ");
}

/** Returns local timezone offset as "+HHMM" / "-HHMM" matching git-filter-repo's raw date format. */
export function formatTzOffset(d: Date): string {
  // getTimezoneOffset returns minutes WEST of UTC. A user in PST has +480.
  const offMinutes = -d.getTimezoneOffset();
  const sign = offMinutes >= 0 ? "+" : "-";
  const abs = Math.abs(offMinutes);
  const hh = String(Math.floor(abs / 60)).padStart(2, "0");
  const mm = String(abs % 60).padStart(2, "0");
  return `${sign}${hh}${mm}`;
}

/** Convert a Date into git-filter-repo's expected (epoch, "+HHMM") tuple. */
export function toEpochTz(d: Date): { epoch: number; tz: string } {
  return { epoch: Math.floor(d.getTime() / 1000), tz: formatTzOffset(d) };
}

/**
 * Build a local Date from Y/M/D/h/m/s fields, returning null when the input is
 * incomplete or impossible. Unlike `new Date(...)`, this REJECTS empty fields (an
 * empty <input type="number"> binds as null → would become year 1900) and
 * out-of-range values (e.g. Feb 30, which would silently normalize to Mar 2) by
 * requiring the resulting Date's components to round-trip. Callers therefore never
 * preview or stage a timestamp different from what the user typed.
 */
export function buildLocalDate(
  y: number,
  mo: number,
  d: number,
  h: number,
  mi: number,
  s: number,
): Date | null {
  for (const n of [y, mo, d, h, mi, s]) {
    if (!Number.isFinite(n)) return null;
  }
  const dt = new Date(y, mo - 1, d, h, mi, s);
  if (
    dt.getFullYear() !== y ||
    dt.getMonth() !== mo - 1 ||
    dt.getDate() !== d ||
    dt.getHours() !== h ||
    dt.getMinutes() !== mi ||
    dt.getSeconds() !== s
  ) {
    return null;
  }
  return dt;
}
