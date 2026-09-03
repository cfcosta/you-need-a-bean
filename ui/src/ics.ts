/**
 * The calendar file the plan card hands out: every payment coming up
 * as an all-day event that repeats on its day of the month, in the
 * iCalendar shape (RFC 5545) that every calendar app imports.
 */

export interface CalendarEvent {
  /** Stable across exports, so a re-import updates the event instead
   * of adding a second one. */
  uid: string;
  /** "YYYY-MM-DD" */
  date: string;
  summary: string;
  description?: string;
  /** The day of the month it repeats on; absent for a one-off. */
  day?: number;
  /** How many times it repeats, `day` given. */
  count?: number;
}

/** How many octets a character takes in UTF-8, which is what the
 * line limit counts. */
const width = (ch: string) => {
  const c = ch.codePointAt(0) ?? 0;
  return c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4;
};

/** A line longer than 75 octets continues on the next line after a
 * space, which counts against that line's 75. Never inside a
 * character. */
export function fold(line: string): string {
  const out: string[] = [];
  let cur = "";
  let used = 0;
  for (const ch of line) {
    const w = width(ch);
    if (used + w > (out.length === 0 ? 75 : 74)) {
      out.push(cur);
      cur = "";
      used = 0;
    }
    cur += ch;
    used += w;
  }
  out.push(cur);
  return out.map((l, i) => (i === 0 ? l : ` ${l}`)).join("\r\n");
}

/** Text with the characters the format reserves escaped. */
export const escapeText = (s: string) =>
  s
    .replace(/\\/g, "\\\\")
    .replace(/;/g, "\\;")
    .replace(/,/g, "\\,")
    .replace(/\r?\n/g, "\\n");

const compact = (date: string) => date.replace(/-/g, "");

/** The file: one VEVENT per payment, stamped with the day it was
 * made. Lines end in CRLF, as the format asks. */
export function calendar(events: CalendarEvent[], stamp: string): string {
  const lines = [
    "BEGIN:VCALENDAR",
    "VERSION:2.0",
    "PRODID:-//you-need-a-bean//liabilities//EN",
    "CALSCALE:GREGORIAN",
  ];
  for (const e of events) {
    lines.push(
      "BEGIN:VEVENT",
      `UID:${e.uid}`,
      `DTSTAMP:${compact(stamp)}T000000Z`,
      `DTSTART;VALUE=DATE:${compact(e.date)}`,
      `SUMMARY:${escapeText(e.summary)}`,
    );
    if (e.description != null) {
      lines.push(`DESCRIPTION:${escapeText(e.description)}`);
    }
    if (e.day != null) {
      const count = e.count != null ? `;COUNT=${e.count}` : "";
      lines.push(`RRULE:FREQ=MONTHLY;BYMONTHDAY=${e.day}${count}`);
    }
    lines.push("END:VEVENT");
  }
  lines.push("END:VCALENDAR");
  return `${lines.map(fold).join("\r\n")}\r\n`;
}
