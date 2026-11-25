// Dates and times, in the format chosen in the settings, "Date and time" in "General"
// (guidelines, 9.18 and 12): one format for the whole application.

import { prefs, type Clock, type DateFormat } from '../git/prefs.svelte';

const two = (n: number) => String(n).padStart(2, '0');
const written = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
const long = new Intl.DateTimeFormat('en-US', { dateStyle: 'full' });

/** The date of `ms` alone: "11-25-2026", "25-11-2026", "2026-11-25" or "Nov 25, 2026". */
export function date(ms: number, format: DateFormat = prefs.dateFormat): string {
  const d = new Date(ms);
  const [day, month, year] = [two(d.getDate()), two(d.getMonth() + 1), String(d.getFullYear())];
  if (format === 'dmy') return `${day}-${month}-${year}`;
  if (format === 'ymd') return `${year}-${month}-${day}`;
  if (format === 'text') return written.format(d);
  return `${month}-${day}-${year}`;
}

/** The time of `ms`, to the second: "14:30:05", or "2:30:05 PM" with AM or PM kept on its line. */
export function time(ms: number, clock: Clock = prefs.clock): string {
  const d = new Date(ms);
  const rest = `${two(d.getMinutes())}:${two(d.getSeconds())}`;
  if (clock === '24') return `${two(d.getHours())}:${rest}`;
  return `${d.getHours() % 12 || 12}:${rest} ${d.getHours() < 12 ? 'AM' : 'PM'}`;
}

/** A moment, on a row: "11-25-2026 14:30:05", or written out "Nov 25, 2026, 14:30:05". */
export function dateTime(ms: number): string {
  const format = prefs.dateFormat;
  return `${date(ms, format)}${format === 'text' ? ', ' : ' '}${time(ms)}`;
}

/** A moment in full, in a tooltip: "Wednesday, November 25, 2026 at 14:30:05". */
export const fullDateTime = (ms: number) => `${long.format(ms)} at ${time(ms)}`;
