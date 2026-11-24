// Dates in the American format of the interface (guidelines, 12): the short date with its time
// on a row, the full date in its tooltip. The time keeps AM or PM on its line.

const short = new Intl.DateTimeFormat('en-US', { dateStyle: 'medium', timeStyle: 'short' });
const full = new Intl.DateTimeFormat('en-US', { dateStyle: 'full', timeStyle: 'short' });
const fullSeconds = new Intl.DateTimeFormat('en-US', { dateStyle: 'full', timeStyle: 'medium' });

const keep = (text: string) => text.replace(/ (AM|PM)$/, '\u00a0$1');

/** "Nov 9, 2026, 11:35 PM", from milliseconds. */
export const dateTime = (ms: number) => keep(short.format(ms));

/** "Monday, November 9, 2026 at 11:35 PM", or with the seconds, "at 11:35:12 PM". */
export const fullDateTime = (ms: number, seconds = false) => keep((seconds ? fullSeconds : full).format(ms));
