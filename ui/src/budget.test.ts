import { describe, expect, test } from "bun:test";

import { lands } from "./budget";

describe("lands", () => {
  test("shows the figure for a category that skipped months", () => {
    expect(lands({ amount: 1200, months: 2 }, 6)).toEqual({
      amount: 1200,
      months: 2,
      of: 6,
    });
  });

  test("says nothing when it lands every month", () => {
    // The mean and the lands-on figure are then the same number, and
    // the table already has a column for it.
    expect(lands({ amount: 300, months: 5 }, 5)).toBeNull();
  });

  test("says nothing when it never landed, or had no window", () => {
    expect(lands(null, 6)).toBeNull();
    expect(lands({ amount: 300, months: 1 }, 0)).toBeNull();
  });
});
