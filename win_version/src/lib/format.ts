/** Number formatting only -- no data types cross this boundary. */

/** `120s -> live`, `1800s -> recent`, otherwise idle. Matches the macOS app. */
const LIVE_WINDOW_MS = 120_000;
const RECENT_WINDOW_MS = 1_800_000;

/** `120.40` */
export function currency(value: number): string {
  return `$${value.toFixed(2)}`;
}

/**
 * Token counts, abbreviated once they stop being readable.
 *
 * 842 -> `842`, 1_200 -> `1.2K`, 3_400_000 -> `3.4M`, 12_300_000_000 -> `12.3B`
 */
export function tokens(value: number): string {
  const n = Math.abs(value);
  if (n >= 1e9) return `${trim(value / 1e9)}B`;
  if (n >= 1e6) return `${trim(value / 1e6)}M`;
  if (n >= 1e3) return `${trim(value / 1e3)}K`;
  return String(Math.round(value));
}

/** One decimal, but never a trailing `.0`. */
function trim(value: number): string {
  const fixed = value.toFixed(1);
  return fixed.endsWith(".0") ? fixed.slice(0, -2) : fixed;
}

/** Grouped digits: 242314082 -> `242,314,082` */
export function number(value: number): string {
  return Math.round(value).toLocaleString("en-US");
}

/** `38.2%` */
export function percent(value: number): string {
  return `${value.toFixed(1)}%`;
}

/**
 * Chart axis labels, in the Chinese units the original dashboard used.
 *
 * 123_456_789 -> `1.2亿`, 123_000 -> `12.3万`, 3_400 -> `3.4k`
 */
export function axis(value: number): string {
  const n = Math.abs(value);
  if (n >= 1e8) return `${(value / 1e8).toFixed(1)}亿`;
  if (n >= 1e7) return `${Math.round(value / 1e4).toLocaleString("en-US")}万`;
  if (n >= 1e4) return `${(value / 1e4).toFixed(1)}万`;
  if (n >= 1e3) return `${(value / 1e3).toFixed(1)}k`;
  return number(value);
}

/** `2026-09-26` -> `09/26`. Returns the input if it is not a plain date. */
export function monthDay(date: string): string {
  const parts = date.split("-");
  return parts.length === 3 ? `${parts[1]}/${parts[2]}` : date;
}

/** RFC3339 with a zone offset, e.g. `+08:00`. */
export function utcOffset(date = new Date()): string {
  const minutes = -date.getTimezoneOffset();
  const sign = minutes < 0 ? "-" : "+";
  const abs = Math.abs(minutes);
  return `${sign}${String(Math.floor(abs / 60)).padStart(2, "0")}:${String(abs % 60).padStart(2, "0")}`;
}

/**
 * "3 分钟前" / "2 小时前", the way the original popover worded it.
 *
 * Relative times are the one place a hand-rolled formatter is worth it: the
 * macOS build leaned on `RelativeDateTimeFormatter`, which has no browser
 * equivalent, and the phrasing differs from `Intl` anyway.
 */
export function relative(timestamp: string | null | undefined): string {
  if (!timestamp) return "—";
  const ms = Date.parse(timestamp);
  if (Number.isNaN(ms)) return "—";
  return relativeFrom(ms);
}

export function relativeFrom(ms: number, now = Date.now()): string {
  const seconds = Math.round((now - ms) / 1000);
  if (seconds < 0) return "刚刚";
  if (seconds < 60) return `${seconds} 秒前`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months} 个月前`;
  return `${Math.floor(months / 12)} 年前`;
}

export type ActivityState = "live" | "recent" | "idle";

/** How recently a session was touched, using the macOS thresholds. */
export function activityState(timestamp: string | null | undefined, now = Date.now()): ActivityState {
  if (!timestamp) return "idle";
  const ms = Date.parse(timestamp);
  if (Number.isNaN(ms)) return "idle";
  const age = now - ms;
  if (age < LIVE_WINDOW_MS) return "live";
  if (age < RECENT_WINDOW_MS) return "recent";
  return "idle";
}

/** `09/26 21:32`, the format the session list uses. */
export function dateTime(timestamp: string | null | undefined): string {
  if (!timestamp) return "—";
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return "—";
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** Human byte size for the database footer. */
export function bytes(value: number): string {
  if (value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const exponent = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** exponent).toFixed(exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}

/** `12.5 MB`, for the provider/model split legend. */
export function shareOf(part: number, whole: number): string {
  if (whole <= 0) return "0.0%";
  return `${((part / whole) * 100).toFixed(1)}%`;
}

/** Which providers this model should be labelled with. */
export function providerLabel(providers: string[]): string {
  return providers.length > 0 ? providers.join(", ") : "未知";
}
