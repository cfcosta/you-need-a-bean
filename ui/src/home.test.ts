import { expect, test } from "bun:test";
import { scenario, accountUrl } from "./home";

test("a cash scenario accrues extra spending with elapsed days", () => {
  const points = [{date:"2026-09-07",balance:1000},{date:"2026-09-17",balance:900}];
  expect(scenario(points,"2026-09-07",30,300).at(-1)).toEqual({date:"2026-10-07",balance:600});
  expect(scenario(points,"2026-09-07",30,300)[1]?.balance).toBe(800);
});
test("an unavailable projection stays unavailable", () => {
  expect(scenario([],"2026-09-07",30,300)).toEqual([]);
});
test("account links retain the date and quote special characters", () => {
  expect(accountUrl("Assets:Bank:Cash","2026-09-07")).toBe("/account/Assets%3ABank%3ACash/2026-09");
});

test("the financial day follows the local calendar across UTC midnight", async () => {
  const {localDay}=await import("./home");
  const old=process.env.TZ; process.env.TZ="America/Sao_Paulo";
  try { expect(localDay(new Date("2026-09-08T01:00:00Z"))).toBe("2026-09-07"); }
  finally { if(old==null)delete process.env.TZ; else process.env.TZ=old; }
});
