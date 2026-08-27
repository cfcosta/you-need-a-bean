import { describe, expect, test } from "bun:test";

import { fireFill, leanMark } from "./fire";

describe("fireFill", () => {
  test("is the progress, as a share of the bar", () => {
    expect(fireFill(0.2)).toBe(0.2);
  });

  test("stops at the end of the bar rather than running past it", () => {
    expect(fireFill(1.8)).toBe(1);
  });

  // A negative net worth is a real state, and a bar drawn backwards
  // from the left edge is not a picture of it.
  test("stays empty when there is nothing to fill it with", () => {
    expect(fireFill(-0.4)).toBe(0);
    expect(fireFill(null)).toBe(0);
  });
});

describe("leanMark", () => {
  test("puts the lean target where it falls on the way to the full one", () => {
    expect(leanMark(25_000, 100_000)).toBe(0.25);
  });

  test("has nowhere to stand without both numbers", () => {
    expect(leanMark(null, 100_000)).toBe(null);
    expect(leanMark(25_000, 0)).toBe(null);
  });

  // A lean number at or past the full one is not a second goalpost:
  // the marker would sit on the end of the bar and read as the target
  // it is standing next to.
  test("drops a marker that would land on the target itself", () => {
    expect(leanMark(100_000, 100_000)).toBe(null);
    expect(leanMark(125_000, 100_000)).toBe(null);
  });

  test("drops a marker with nothing behind it", () => {
    expect(leanMark(0, 100_000)).toBe(null);
  });
});
