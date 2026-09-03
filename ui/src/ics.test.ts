import { describe, expect, test } from "bun:test";

import { calendar, escapeText, fold } from "./ics";

const octets = (s: string) => new TextEncoder().encode(s).length;

describe("calendar", () => {
  test("is one repeating all-day event per payment", () => {
    const text = calendar(
      [
        {
          uid: "C@you-need-a-bean",
          date: "2026-06-15",
          summary: "Car: $ 400 due",
          description: "The usual payment on the loan; $ 8,600.00 left after it.",
          day: 15,
          count: 24,
        },
      ],
      "2026-06-10",
    );
    expect(text.split("\r\n")).toEqual([
      "BEGIN:VCALENDAR",
      "VERSION:2.0",
      "PRODID:-//you-need-a-bean//liabilities//EN",
      "CALSCALE:GREGORIAN",
      "BEGIN:VEVENT",
      "UID:C@you-need-a-bean",
      "DTSTAMP:20260610T000000Z",
      "DTSTART;VALUE=DATE:20260615",
      "SUMMARY:Car: $ 400 due",
      "DESCRIPTION:The usual payment on the loan\\; $ 8\\,600.00 left after it.",
      "RRULE:FREQ=MONTHLY;BYMONTHDAY=15;COUNT=24",
      "END:VEVENT",
      "END:VCALENDAR",
      "",
    ]);
  });

  test("an event with no day happens once", () => {
    const text = calendar(
      [{ uid: "x", date: "2026-06-10", summary: "Send the extra" }],
      "2026-06-10",
    );
    expect(text).not.toContain("RRULE");
    expect(text).not.toContain("DESCRIPTION");
    expect(text).toContain("\r\nSUMMARY:Send the extra\r\n");
  });

  test("escapes what the format reserves", () => {
    expect(escapeText("a\\b;c,d\ne")).toBe("a\\\\b\\;c\\,d\\ne");
  });

  test("folds long lines at 75 octets without splitting a character", () => {
    const line = `SUMMARY:${"é".repeat(60)}`;
    const lines = fold(line).split("\r\n");
    expect(lines.length).toBeGreaterThan(1);
    expect(lines.every((l) => octets(l) <= 75)).toBe(true);
    expect(lines.slice(1).every((l) => l.startsWith(" "))).toBe(true);
    expect(lines.map((l, i) => (i === 0 ? l : l.slice(1))).join("")).toBe(line);
    expect(lines[0]).toBe(`SUMMARY:${"é".repeat(33)}`);
    expect(fold("SUMMARY:short")).toBe("SUMMARY:short");
  });
});
