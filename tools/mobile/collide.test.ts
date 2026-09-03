import { describe, expect, it } from "bun:test";
import { collide, type Box } from "./collide";

const box = (left: number, top: number, w: number, h: number): Box => ({
  left,
  right: left + w,
  top,
  bottom: top + h,
});

describe("collide", () => {
  it("says nothing when two boxes sit side by side", () => {
    expect(collide([box(0, 0, 50, 14)], [box(60, 0, 50, 14)])).toBe(0);
  });

  it("reports how many pixels two boxes share", () => {
    expect(collide([box(0, 0, 50, 14)], [box(30, 0, 50, 14)])).toBe(20);
  });

  it("forgives a shared edge and a hairline of rounding", () => {
    expect(collide([box(0, 0, 50, 14)], [box(50, 0, 50, 14)])).toBe(0);
    expect(collide([box(0, 0, 50, 14)], [box(48, 0, 50, 14)])).toBe(0);
  });

  it("says nothing when boxes are on different lines", () => {
    expect(collide([box(0, 0, 50, 14)], [box(0, 20, 50, 14)])).toBe(0);
  });

  // The reason this function exists. A run of text that wraps has one
  // box per line; the rectangle bounding both spans the full width of
  // the paragraph and would appear to sit on top of every neighbour
  // sharing those lines, none of which it actually touches.
  it("does not accuse a wrapped run of hitting text it wraps around", () => {
    const wrapped = [box(338, 0, 37, 14), box(45, 17, 19, 14)];
    const neighbour = [box(45, 0, 280, 14)];
    expect(collide(wrapped, neighbour)).toBe(0);
  });

  it("still catches a real hit on one line of a wrapped run", () => {
    const wrapped = [box(338, 0, 37, 14), box(45, 17, 19, 14)];
    const onTop = [box(50, 17, 30, 14)];
    expect(collide(wrapped, onTop)).toBe(14);
  });

  it("reports the worst of several shared pairs", () => {
    const a = [box(0, 0, 50, 14), box(0, 17, 50, 14)];
    const b = [box(40, 0, 50, 14), box(10, 17, 50, 14)];
    expect(collide(a, b)).toBe(40);
  });
});
