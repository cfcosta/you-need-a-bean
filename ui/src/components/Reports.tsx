import type {
  CashflowPoint,
  Growth,
  GrowthPoint,
  Income,
  Movers,
  Payees,
  NetWorthPoint,
  Projects,
  ReportsView,
  Season,
  Trust,
  YearGroup,
} from "../api";
import {
  calendarMonth,
  fmt,
  fmtCompact,
  monthName,
  monthShort,
  windowLabel,
} from "../format";
import type { RibbonMonth, StripCell } from "../year";
import { foldGroups, yearRibbon, yearStrip } from "../year";

/** "2026-08" plus n months. */
function addMonths(m: string, n: number): string {
  const total = Number(m.slice(0, 4)) * 12 + (Number(m.slice(5, 7)) - 1) + n;
  const y = Math.floor(total / 12);
  return `${y}-${String(total - y * 12 + 1).padStart(2, "0")}`;
}

/** 27 → "2 yr 3 mo" */
function duration(months: number): string {
  const y = Math.floor(months / 12);
  const m = months % 12;
  if (y === 0) return `${m} mo`;
  if (m === 0) return `${y} yr`;
  return `${y} yr ${m} mo`;
}

/** How long the current stash takes on its own, with nothing more saved. */
function coastLabel(months: number | null | undefined): string {
  if (months == null) return "never on its own";
  if (months === 0) return "already there";
  return duration(months);
}

/** ["ADA"] → "ADA"; ["ADA","SOL","BTC"] → "ADA, SOL and BTC". */
function listOf(items: string[]): string {
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}

const monthYear = (m: string) => `${monthShort(m)} ${m.slice(0, 4)}`;

/** A change in money, always carrying its sign. `fmtCompact` already
 * writes the minus, so only a rise needs one added. */
const signed = (v: number, cur: string) =>
  `${v > 0 ? "+" : ""}${fmtCompact(v, cur)}`;

/** A change as a percentage. `null` means there was nothing to grow
 * from, which is a category that appeared rather than one that rose. */
function changePct(ratio: number | null): string {
  if (ratio == null) return "new";
  const p = ratio * 100;
  if (Math.abs(p) < 0.5) return "flat";
  return `${p > 0 ? "+" : "\u2212"}${Math.round(Math.abs(p))}%`;
}

/** Round gridline values covering [min, max], roughly `want` of them. */
function ticks(min: number, max: number, want: number): number[] {
  const raw = (max - min) / Math.max(1, want);
  if (!(raw > 0)) return [];
  const mag = 10 ** Math.floor(Math.log10(raw));
  const r = raw / mag;
  const step = (r >= 5 ? 10 : r >= 2 ? 5 : r >= 1 ? 2 : 1) * mag;
  const out: number[] = [];
  for (let v = Math.ceil(min / step) * step; v <= max; v += step) out.push(v);
  return out;
}

/** Pad [min, max] so marks don't touch the frame; always spans zero. */
function bounds(values: number[]): [number, number] {
  let min = Math.min(0, ...values);
  let max = Math.max(0, ...values);
  if (max === min) max = min + 1;
  const head = (max - min) * 0.08;
  if (min < 0) min -= head;
  return [min, max + head];
}

function NetWorthChart({
  points,
  cur,
}: {
  points: NetWorthPoint[];
  cur: string;
}) {
  const W = 720;
  const H = 210;
  const TOP = 16;
  const BOT = 20;
  const PAD = 8;
  const n = points.length;
  const last = points[n - 1];
  if (last == null) return <div className="empty">no months to chart</div>;
  // The stack has to fit inside the frame, not just the net line: gross
  // assets sit above it and debt below.
  const [min, max] = bounds(
    points.flatMap((p) => [p.net, p.assets, p.liabilities]),
  );
  const x = (i: number) =>
    n === 1 ? W / 2 : PAD + (i * (W - 2 * PAD)) / (n - 1);
  const y = (v: number) => TOP + ((max - v) * (H - TOP - BOT)) / (max - min);
  const line = points
    .map(
      (p, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(p.net).toFixed(1)}`,
    )
    .join("");

  /** A filled band between two per-point levels: out along the top,
   * back along the bottom. */
  const band = (
    lo: (p: NetWorthPoint) => number,
    hi: (p: NetWorthPoint) => number,
  ) => {
    const out = points
      .map(
        (p, i) =>
          `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(hi(p)).toFixed(1)}`,
      )
      .join("");
    const back = points
      .map((p, i) => ({ p, i }))
      .reverse()
      .map(({ p, i }) => `L${x(i).toFixed(1)},${y(lo(p)).toFixed(1)}`)
      .join("");
    return `${out}${back}Z`;
  };
  const zero = () => 0;
  const anyHoldings = points.some((p) => Math.abs(p.holdings) > 0.005);
  const anyDebt = points.some((p) => Math.abs(p.liabilities) > 0.005);

  // Years as x labels on long ranges, every third month on short ones.
  const labels: { i: number; text: string }[] = [];
  if (n > 18) {
    const years = points.filter((p) => p.month.endsWith("-01")).length;
    const every = years > 8 ? 2 : 1;
    let seen = 0;
    points.forEach((p, i) => {
      if (p.month.endsWith("-01") && seen++ % every === 0) {
        labels.push({ i, text: p.month.slice(0, 4) });
      }
    });
  } else {
    points.forEach((p, i) => {
      if (i % 3 === 0) labels.push({ i, text: monthShort(p.month) });
    });
  }
  const slot = n > 1 ? (W - 2 * PAD) / (n - 1) : W;

  return (
    <>
      <svg
        className="chart-svg"
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={`Net worth by month, converted to ${cur}`}
      >
        {ticks(min, max, 3)
          .filter((v) => Math.abs(v) > 1e-9)
          .map((v) => (
            <g key={v}>
              <line x1={PAD} x2={W - PAD} y1={y(v)} y2={y(v)} className="grid" />
              <text x={PAD} y={y(v) - 3} className="axis">
                {fmtCompact(v, cur)}
              </text>
            </g>
          ))}
        <path d={band(zero, (p) => p.cash)} className="nw-cash" />
        {anyHoldings && (
          <path
            d={band(
              (p) => p.cash,
              (p) => p.assets,
            )}
            className="nw-holdings"
          />
        )}
        {anyDebt && (
          <path d={band((p) => p.liabilities, zero)} className="nw-debt" />
        )}
        <line x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} className="grid zero" />
        <path d={line} className="nw-line" />
        <circle cx={x(n - 1)} cy={y(last.net)} r="3" className="nw-dot" />
        <text x={W - PAD} y={y(last.net) - 7} textAnchor="end" className="cap">
          {fmtCompact(last.net, cur)}
        </text>
        {labels.map((l) => (
          <text
            key={l.i}
            x={x(l.i)}
            y={H - 5}
            textAnchor="middle"
            className="axis"
          >
            {l.text}
          </text>
        ))}
        {points.map((p, i) => (
          <rect
            key={p.month}
            x={x(i) - slot / 2}
            y={0}
            width={slot}
            height={H}
            fill="transparent"
          >
            <title>{`${monthName(p.month)}\nnet ${fmt(p.net, cur)}\ncash ${fmt(p.cash, cur)} · holdings ${fmt(p.holdings, cur)} · debt ${fmt(p.liabilities, cur)}`}</title>
          </rect>
        ))}
      </svg>
      <div className="key">
        <span className="key-item">
          <i className="sw cash" />
          cash {fmtCompact(last.cash, cur)}
        </span>
        {anyHoldings && (
          <span className="key-item">
            <i className="sw holdings" />
            holdings {fmtCompact(last.holdings, cur)}
          </span>
        )}
        {anyDebt && (
          <span className="key-item">
            <i className="sw debt" />
            debt {fmtCompact(last.liabilities, cur)}
          </span>
        )}
      </div>
    </>
  );
}

function CashflowChart({
  points,
  cur,
}: {
  points: CashflowPoint[];
  cur: string;
}) {
  const W = 720;
  const H = 170;
  const TOP = 16;
  const BOT = 20;
  const PAD = 8;
  const n = points.length;
  if (n === 0) return <div className="empty">no months to chart</div>;
  const [min, max] = bounds(points.map((p) => p.net));
  const y = (v: number) => TOP + ((max - v) * (H - TOP - BOT)) / (max - min);
  const slot = (W - 2 * PAD) / n;
  const bw = Math.max(2, Math.min(26, slot - 2));

  return (
    <svg
      className="chart-svg"
      viewBox={`0 0 ${W} ${H}`}
      role="img"
      aria-label={`Monthly cashflow, converted to ${cur}`}
    >
      {ticks(min, max, 3)
        .filter((v) => Math.abs(v) > 1e-9)
        .map((v) => (
          <g key={v}>
            <line x1={PAD} x2={W - PAD} y1={y(v)} y2={y(v)} className="grid" />
            <text x={PAD} y={y(v) - 3} className="axis">
              {fmtCompact(v, cur)}
            </text>
          </g>
        ))}
      <line x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} className="grid zero" />
      {points.map((p, i) => {
        const cx = PAD + i * slot + slot / 2;
        const h = Math.max(1.5, Math.abs(y(p.net) - y(0)));
        return (
          <g key={p.month}>
            <rect
              x={cx - bw / 2}
              y={p.net >= 0 ? y(0) - h : y(0)}
              width={bw}
              height={h}
              rx="2"
              className={p.net >= 0 ? "cf-bar pos" : "cf-bar neg"}
            />
            <rect
              x={cx - slot / 2}
              y={0}
              width={slot}
              height={H}
              fill="transparent"
            >
              <title>{`${monthName(p.month)}\nin ${fmt(p.income, cur)} · out ${fmt(p.expenses, cur)}\nnet ${fmt(p.net, cur)}`}</title>
            </rect>
            {i % 3 === 0 && (
              <text x={cx} y={H - 5} textAnchor="middle" className="axis">
                {monthShort(p.month)}
                {p.month.slice(5, 7) === "01" ? ` ${p.month.slice(2, 4)}` : ""}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}

interface SavingsPoint {
  month: string;
  income: number;
  net: number;
  /** net / income; `null` when the month brought no income. */
  rate: number | null;
  /** 3-month aggregate rate: Σnet / Σincome over the trailing three. */
  trend: number | null;
}

function savingsSeries(points: CashflowPoint[]): SavingsPoint[] {
  return points.map((p, i) => {
    const win = points.slice(Math.max(0, i - 2), i + 1);
    const inc3 = win.reduce((s, q) => s + q.income, 0);
    const net3 = win.reduce((s, q) => s + q.net, 0);
    return {
      month: p.month,
      income: p.income,
      net: p.net,
      rate: p.income > 0 ? p.net / p.income : null,
      trend: inc3 > 0 ? net3 / inc3 : null,
    };
  });
}

function SavingsRateChart({
  points,
  cur,
}: {
  points: SavingsPoint[];
  cur: string;
}) {
  const W = 460;
  const H = 170;
  const TOP = 16;
  const BOT = 20;
  const PAD = 8;
  const n = points.length;
  // Geometry is clamped to ±100% so one weird month can't flatten the
  // rest; tooltips keep the true number.
  const clamp = (v: number) => Math.max(-1, Math.min(1, v));
  if (n === 0 || points.every((p) => p.rate == null)) {
    return <div className="empty">no income months to chart</div>;
  }
  const values = points.flatMap((p) => [
    ...(p.rate != null ? [clamp(p.rate)] : []),
    ...(p.trend != null ? [clamp(p.trend)] : []),
  ]);
  const [min, max] = bounds(values);
  const y = (v: number) => TOP + ((max - v) * (H - TOP - BOT)) / (max - min);
  const slot = (W - 2 * PAD) / n;
  const bw = Math.max(2, Math.min(22, slot - 2));
  const pct = (v: number) => `${Math.round(v * 100)}%`;

  // The trend line breaks where three straight months bring no income.
  let line = "";
  let pen = false;
  points.forEach((p, i) => {
    if (p.trend == null) {
      pen = false;
      return;
    }
    const cx = PAD + i * slot + slot / 2;
    line += `${pen ? "L" : "M"}${cx.toFixed(1)},${y(clamp(p.trend)).toFixed(1)}`;
    pen = true;
  });

  return (
    <svg
      className="chart-svg"
      viewBox={`0 0 ${W} ${H}`}
      role="img"
      aria-label="Savings rate by month"
    >
      {ticks(min, max, 3)
        .filter((v) => Math.abs(v) > 1e-9)
        .map((v) => (
          <g key={v}>
            <line x1={PAD} x2={W - PAD} y1={y(v)} y2={y(v)} className="grid" />
            <text x={PAD} y={y(v) - 3} className="axis">
              {pct(v)}
            </text>
          </g>
        ))}
      <line x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} className="grid zero" />
      {points.map((p, i) => {
        const cx = PAD + i * slot + slot / 2;
        const r = p.rate;
        // Past −1000% the percentage is noise; the amounts below tell
        // the story.
        const label =
          r == null
            ? "no income this month"
            : r < -10
              ? "spending dwarfed income"
              : `saved ${pct(r)} of income`;
        const tip = `${monthName(p.month)}\n${label}\nin ${fmt(p.income, cur)} · net ${fmt(p.net, cur)}`;
        return (
          <g key={p.month}>
            {r != null && (
              <rect
                x={cx - bw / 2}
                y={r >= 0 ? y(clamp(r)) : y(0)}
                width={bw}
                height={Math.max(1.5, Math.abs(y(clamp(r)) - y(0)))}
                rx="2"
                className={`cf-bar ${r >= 0 ? "pos" : "neg"}${
                  r === clamp(r) ? "" : " clip"
                }`}
              />
            )}
            <rect
              x={cx - slot / 2}
              y={0}
              width={slot}
              height={H}
              fill="transparent"
            >
              <title>{tip}</title>
            </rect>
            {i % 3 === 0 && (
              <text x={cx} y={H - 5} textAnchor="middle" className="axis">
                {monthShort(p.month)}
                {p.month.slice(5, 7) === "01" ? ` ${p.month.slice(2, 4)}` : ""}
              </text>
            )}
          </g>
        );
      })}
      <path d={line} className="sr-line" />
    </svg>
  );
}

function GrowthChart({
  points,
  cur,
  gaps,
}: {
  points: GrowthPoint[];
  cur: string;
  /** Whether missing prices are riding along in the residual. */
  gaps: boolean;
}) {
  const W = 460;
  const H = 170;
  const TOP = 16;
  const BOT = 20;
  const PAD = 8;
  const n = points.length;
  if (n === 0) return <div className="empty">no months to chart</div>;
  // Each bar stacks its parts away from zero in whichever direction
  // each one points, so a month's reach is the sum per side.
  const [min, max] = bounds(
    points.flatMap((p) => {
      const parts = [p.saved, p.market];
      const side = (keep: (v: number) => boolean) =>
        parts.filter(keep).reduce((a, b) => a + b, 0);
      return [side((v) => v > 0), side((v) => v < 0)];
    }),
  );
  const y = (v: number) => TOP + ((max - v) * (H - TOP - BOT)) / (max - min);
  const slot = (W - 2 * PAD) / n;
  const bw = Math.max(2, Math.min(22, slot - 2));

  return (
    <>
      <svg
        className="chart-svg"
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={`What moved net worth each month, converted to ${cur}`}
      >
        {ticks(min, max, 3)
          .filter((v) => Math.abs(v) > 1e-9)
          .map((v) => (
            <g key={v}>
              <line x1={PAD} x2={W - PAD} y1={y(v)} y2={y(v)} className="grid" />
              <text x={PAD} y={y(v) - 3} className="axis">
                {fmtCompact(v, cur)}
              </text>
            </g>
          ))}
        <line x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} className="grid zero" />
        {points.map((p, i) => {
          const cx = PAD + i * slot + slot / 2;
          // Running offsets: positives pile up, negatives pile down.
          let up = 0;
          let down = 0;
          const bars = (
            [
              ["saved", p.saved],
              ["market", p.market],
            ] as const
          )
            .filter(([, v]) => Math.abs(v) > 0.005)
            .map(([kind, v]) => {
              const base = v >= 0 ? up : down;
              const tip = base + v;
              if (v >= 0) up = tip;
              else down = tip;
              return { kind, y0: y(base), y1: y(tip) };
            });
          const tip = [
            monthName(p.month),
            `net worth ${p.delta >= 0 ? "+" : "−"}${fmt(Math.abs(p.delta), cur)}`,
            `saved ${fmt(p.saved, cur)} · market ${fmt(p.market, cur)}`,
            ...(p.equity !== 0 ? [`capital in ${fmt(p.equity, cur)}`] : []),
          ].join("\n");
          return (
            <g key={p.month}>
              {bars.map((b) => (
                <rect
                  key={b.kind}
                  x={cx - bw / 2}
                  y={Math.min(b.y0, b.y1)}
                  width={bw}
                  height={Math.max(1.5, Math.abs(b.y1 - b.y0))}
                  rx="2"
                  className={`gr-bar ${b.kind}`}
                />
              ))}
              <rect
                x={cx - slot / 2}
                y={0}
                width={slot}
                height={H}
                fill="transparent"
              >
                <title>{tip}</title>
              </rect>
              {i % 3 === 0 && (
                <text x={cx} y={H - 5} textAnchor="middle" className="axis">
                  {monthShort(p.month)}
                  {p.month.slice(5, 7) === "01" ? ` ${p.month.slice(2, 4)}` : ""}
                </text>
              )}
            </g>
          );
        })}
      </svg>
      <div className="key">
        <span className="key-item">
          <i className="sw saved" />
          you saved
        </span>
        <span className="key-item">
          <i className="sw market" />
          {gaps ? "market & gaps" : "markets moved"}
        </span>
      </div>
    </>
  );
}

function GrowthCard({ growth, cur }: { growth: Growth; cur: string }) {
  const points = growth.points.slice(-24);
  const pct = (v: number) =>
    `${v >= 0 ? "" : "−"}${Math.abs(v * 100).toFixed(1)}%`;
  // With prices missing, the residual is not the market alone, and
  // saying otherwise would be the most misleading number on the page.
  const gaps = growth.unpriced.length > 0;
  const residual = gaps ? "Market & gaps" : "Markets moved";

  return (
    <div className="report-card">
      <h2>What moved net worth</h2>
      <div className="sub2">
        saving vs markets, {windowLabel(growth.window)}
        {points.length < growth.points.length
          ? ` · chart shows last ${points.length} months`
          : ""}
      </div>
      <div className="fire-stats gr-stats">
        <div className="fire-stat">
          <div className="lbl">You saved</div>
          <div className={`v num ${growth.saved >= 0 ? "ok" : "bad"}`}>
            {fmt(growth.saved, cur, 0)}
          </div>
        </div>
        <div className="fire-stat">
          <div className="lbl">{residual}</div>
          <div className={`v num ${growth.market >= 0 ? "ok" : "bad"}`}>
            {fmt(growth.market, cur, 0)}
          </div>
        </div>
        <div className="fire-stat">
          <div className="lbl">Return on the pot</div>
          <div
            className="v num"
            title={
              gaps
                ? `not reported while ${listOf(growth.unpriced)} have no price`
                : undefined
            }
          >
            {growth.implied_return != null ? pct(growth.implied_return) : "—"}
          </div>
        </div>
      </div>
      <GrowthChart points={points} cur={cur} gaps={gaps} />
      <div className="fine">
        Anything net worth did that no transaction explains is counted as the
        market: prices and exchange rates moving under what you already hold.
        {gaps
          ? ` Here it also carries the gaps left by ${listOf(growth.unpriced)}, which have no price — selling one for cash looks like a gain nothing caused.`
          : ""}
        {growth.equity !== 0
          ? ` ${fmt(growth.equity, cur, 0)} arrived through Equity accounts — opening balances and other capital from outside the ledger — and is left out of both bars.`
          : ""}
      </div>
    </div>
  );
}

/** Where the year went: the whole year as a ribbon of months, then
 * every group as its own strip under the same axis. The dashed line
 * means the same thing in both — what a usual month costs — so a bar
 * standing above it reads as a month that cost more than it should
 * without anything having to say so. */
function YearCard({ data, cur }: { data: ReportsView; cur: string }) {
  const { months, groups, typical } = data.year;
  const total = groups.reduce((s, g) => s + g.total, 0);
  // Past nine rows the strips stop being readable, so the tail folds
  // into one — summed month by month, because where the rest of the
  // year's money went is still worth seeing even in aggregate.
  const rest = groups.slice(9);
  const rows: [YearGroup, boolean][] = groups
    .slice(0, 9)
    .map((g) => [g, false] as [YearGroup, boolean]);
  if (rest.length > 0) {
    const name = `${rest.length} more groups`;
    rows.push([foldGroups(name, rest, months.length), true]);
  }

  // The prior year is all-or-nothing: the core withholds it entirely
  // rather than compare a whole year against however much history
  // happens to precede it.
  const compared = data.year.prior_window != null;
  const priorTotal = groups.reduce((s, g) => s + (g.prior ?? 0), 0);
  const change = (g: { total: number; prior: number | null }) =>
    g.prior == null || g.prior <= 0 ? null : g.total / g.prior - 1;
  const share = (v: number) => {
    const p = (v / total) * 100;
    return p >= 0.5 ? `${Math.round(p)}%` : "<1%";
  };

  const ribbon = yearRibbon(months, typical);
  const height = (h: number) => `${Math.max(h * 100, 2)}%`;

  const monthTitle = (m: RibbonMonth) =>
    `${monthName(m.month)}: ${fmt(m.total, cur)}` +
    (typical == null ? "" : ` · a usual month is ${fmt(typical, cur)}`) +
    (m.prior == null ? "" : ` · ${fmt(m.prior, cur)} a year earlier`);

  const cellTitle = (name: string, c: StripCell, usual: number | null) =>
    `${name}, ${monthName(c.month)}: ${fmt(c.value, cur)}` +
    (usual == null ? "" : ` · usually ${fmt(usual, cur)}`);

  const rowTitle = (g: YearGroup) =>
    `${g.name}: ${fmt(g.total, cur)} over the year` +
    (g.typical == null ? "" : ` · ${fmt(g.typical, cur)} in a usual month`) +
    (g.prior == null
      ? ""
      : ` · ${fmt(g.prior, cur)} the year before (${signed(g.total - g.prior, cur)})`) +
    ` · ${share(g.total)} of the year`;

  return (
    <div className="report-card">
      <div className="yr-head">
        <div>
          <h2>Where the year went</h2>
          <div className="yr-range">{windowLabel(data.year.window)}</div>
        </div>
        <div className="yr-fig">
          <span className="yr-total num">{fmtCompact(total, cur)}</span>
          {compared && (
            <span
              className={`yr-delta ${total > priorTotal ? "up" : "down"}`}
              title={`${fmt(priorTotal, cur)} over ${windowLabel(data.year.prior_window)}`}
            >
              {changePct(priorTotal > 0 ? total / priorTotal - 1 : null)} on{" "}
              {fmtCompact(priorTotal, cur)}
            </span>
          )}
        </div>
      </div>
      {groups.length === 0 ? (
        <div className="empty">no spending in the last year</div>
      ) : (
        <>
          <div className="yr-ribbon">
            {ribbon.months.map((m) => (
              <span
                key={m.month}
                className={`yr-mon${m.yearStart ? " turn" : ""}`}
                title={monthTitle(m)}
              >
                <span className="yr-mon-track">
                  {m.total > 0 && (
                    <span
                      className={`yr-mon-bar${m.above ? " above" : ""}`}
                      style={{ height: height(m.height) }}
                    />
                  )}
                  {/* A segment per month rather than one line across
                      the ribbon: the tracks are what it has to measure
                      against, and the month labels below them would
                      drag a single line off its own scale. */}
                  {ribbon.typicalHeight != null && (
                    <span
                      className="yr-usual"
                      style={{ bottom: `${ribbon.typicalHeight * 100}%` }}
                    />
                  )}
                  {m.priorHeight != null && m.prior !== null && m.prior > 0 && (
                    <span
                      className="yr-was"
                      style={{ bottom: `${m.priorHeight * 100}%` }}
                    />
                  )}
                </span>
                <span className="yr-mon-tag">
                  {m.yearStart ? m.month.slice(2, 4) : monthShort(m.month)[0]}
                </span>
              </span>
            ))}
          </div>
          <div className="yr-rows">
            {rows.map(([g, other]) => {
              const strip = yearStrip(g, months);
              return (
                <div
                  key={g.name}
                  className={`yr-row${other ? " other" : ""}`}
                  title={rowTitle(g)}
                >
                  <span className="yr-name">{g.name}</span>
                  <span className="yr-strip">
                    {strip.typicalHeight != null && (
                      <span
                        className="yr-usual"
                        style={{ bottom: `${strip.typicalHeight * 100}%` }}
                      />
                    )}
                    {strip.cells.map((c) => (
                      <span
                        key={c.month}
                        className="yr-cell"
                        title={cellTitle(g.name, c, g.typical)}
                      >
                        {c.value > 0 && (
                          <span
                            className={`yr-cell-bar${c.above ? " above" : ""}`}
                            style={{ height: height(c.height) }}
                          />
                        )}
                      </span>
                    ))}
                  </span>
                  <span className="yr-amt num">{fmtCompact(g.total, cur)}</span>
                  {compared ? (
                    <span
                      className={`yr-pct num ${g.total > (g.prior ?? 0) ? "up" : g.total < (g.prior ?? 0) ? "down" : ""}`}
                    >
                      {changePct(change(g))}
                    </span>
                  ) : (
                    <span className="yr-pct num">{share(g.total)}</span>
                  )}
                </div>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}

function MoversCard({ data, cur }: { data: Movers; cur: string }) {
  const delta = data.recent_total - data.prior_total;
  // Ranked by money, so the widest bar is the biggest move either way.
  const max = Math.max(...data.items.map((i) => Math.abs(i.delta)), 1);
  const width = (v: number) => `${(Math.abs(v) / max) * 50}%`;

  return (
    <div className="report-card">
      <h2>What changed</h2>
      <div className="sub2">
        {data.recent == null ? (
          "two quarters of history to compare"
        ) : (
          <>
            {windowLabel(data.recent)} against {windowLabel(data.prior)} ·{" "}
            <b className={delta > 0 ? "up" : "down"}>{signed(delta, cur)}</b> in
            all
          </>
        )}
      </div>
      {data.items.length === 0 ? (
        <div className="empty">
          {data.recent == null
            ? "not enough history yet — six months are needed to compare two quarters"
            : "no category moved enough to be worth naming"}
        </div>
      ) : (
        <div className="mv-rows">
          {data.items.map((i) => (
            <div
              key={i.account}
              className="mv-row"
              title={`${i.account}: ${fmt(i.prior, cur)} → ${fmt(i.recent, cur)}`}
            >
              <span className="mv-name">
                {i.label}
                {i.group && <span className="mv-group">{i.group}</span>}
              </span>
              <span className="mv-track">
                <span
                  className={`mv-bar ${i.delta > 0 ? "up" : "down"}`}
                  style={
                    i.delta > 0
                      ? { left: "50%", width: width(i.delta) }
                      : { right: "50%", width: width(i.delta) }
                  }
                />
              </span>
              <span className={`mv-amt num ${i.delta > 0 ? "up" : "down"}`}>
                {signed(i.delta, cur)}
              </span>
              <span className="mv-pct num">{changePct(i.ratio)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** "Mar 2026", or "Jan – Mar 2026" once a topic spans months. */
function spanLabel(first: string, last: string): string {
  if (first.slice(0, 7) === last.slice(0, 7)) return monthYear(first);
  if (first.slice(0, 4) === last.slice(0, 4)) {
    return `${monthShort(first)} \u2013 ${monthYear(last)}`;
  }
  return `${monthYear(first)} \u2013 ${monthYear(last)}`;
}

function ProjectsCard({ data, cur }: { data: Projects; cur: string }) {
  const top = data.items.slice(0, 10);
  const rest = data.items.length - top.length;
  // Two shapes can't be topics under any reading, and saying how many
  // were set aside beats a card that looks like the whole story.
  const left = [
    data.singletons > 0 &&
      `${data.singletons} name${data.singletons === 1 ? "" : "s"} on one transaction`,
    data.markers > 0 && `${data.markers} that moved no money`,
  ].filter(Boolean) as string[];

  return (
    <div className="report-card">
      <h2>Projects</h2>
      <div className="sub2">what each tag and link cost, across its whole run</div>
      {data.items.length === 0 ? (
        <div className="empty">no tag or link spans more than one transaction</div>
      ) : (
        <div className="pj-rows">
          {top.map((p) => (
            <div
              key={`${p.sigil}${p.name}`}
              className="pj-row"
              title={`${p.sigil}${p.name}: ${fmt(p.spent, cur)} spent${
                p.income > 0 ? `, ${fmt(p.income, cur)} back` : ""
              } · ${p.first} to ${p.last}`}
            >
              <span className="pj-name">
                <i>{p.sigil}</i>
                {p.name}
              </span>
              <span className="pj-meta">
                <span>{spanLabel(p.first, p.last)}</span>
                <span>
                  {p.count} txns · {p.categories}{" "}
                  {p.categories === 1 ? "category" : "categories"}
                </span>
                {p.income > 0 && (
                  <b className="down" title={`${fmt(p.spent, cur)} charged`}>
                    {fmtCompact(p.income, cur)} back
                  </b>
                )}
              </span>
              <span className="pj-amt num">{fmt(p.net, cur, 0)}</span>
            </div>
          ))}
          {rest > 0 && <div className="pj-note">{rest} more</div>}
        </div>
      )}
      {left.length > 0 && (
        <div className="pj-note">left out: {left.join(", ")}</div>
      )}
    </div>
  );
}

function IncomeCard({ data, cur }: { data: Income; cur: string }) {
  const max = data.sources[0]?.total ?? 0;
  const width = (v: number) => (max > 0 ? `${(v / max) * 100}%` : "0%");
  const pct = (v: number | null) =>
    v == null ? "\u2014" : v < 0.005 ? "<1%" : `${Math.round(v * 100)}%`;
  const top = data.sources.slice(0, 8);
  const rest = data.sources.length - top.length;

  return (
    <div className="report-card">
      <h2>Where income comes from</h2>
      <div className="sub2">
        by source, {windowLabel(data.window)} · {fmt(data.total, cur, 0)}{" "}
        in all
        {data.effective_sources != null && (
          <>
            {" \u00b7 "}
            <span
              title="1 / the sum of each source's squared share: how many equally-sized sources this spread is worth. Two jobs paying the same is 2.0."
            >
              {data.effective_sources.toFixed(1)} effective
            </span>
          </>
        )}
      </div>

      {/* The Coast and Barista question: how much of the bill is
          already covered by money that arrives without you. */}
      <div className="ic-passive">
        {data.passive > 0 ? (
          <>
            <span className="num">{pct(data.passive_cover)}</span>
            <span>
              {" "}
              of what you spend already pays itself ·{" "}
              {fmt(data.passive, cur, 0)} passive, {pct(data.passive_share)} of
              income
            </span>
          </>
        ) : (
          <span className="muted">
            nothing here arrives without work — mark a source with{" "}
            <code>income: "passive"</code> on its open directive
          </span>
        )}
      </div>

      {data.sources.length === 0 ? (
        <div className="empty">no income in the last year</div>
      ) : (
        <div className="ic-rows">
          {top.map((s) => (
            <div
              key={s.name}
              className="ic-row"
              title={`${s.name}: ${fmt(s.total, cur)}${
                s.passive > 0 ? ` \u00b7 ${fmt(s.passive, cur)} passive` : ""
              }`}
            >
              <span className="ic-name">{s.name}</span>
              <span className="ic-track">
                <span className="ic-bar" style={{ width: width(s.total) }} />
                {s.passive > 0 && (
                  <span
                    className="ic-bar passive"
                    style={{ width: width(s.passive) }}
                  />
                )}
              </span>
              <span className="ic-amt num">{fmtCompact(s.total, cur)}</span>
              <span className="ic-pct num">{pct(s.share)}</span>
            </div>
          ))}
          {rest > 0 && <div className="pj-note">{rest} more</div>}
        </div>
      )}

      {data.inferred > 0 && (
        <div className="pj-note">
          {data.inferred} source{data.inferred === 1 ? "" : "s"} read as passive
          from {data.inferred === 1 ? "its name" : "their names"} alone
          {data.declared > 0 && `, ${data.declared} marked in the ledger`}
        </div>
      )}
    </div>
  );
}

/** What a year of spending looks like when the level is taken out, and
 * where this one lands if it keeps to that shape. The trailing average
 * calls December a surprise every December; the median across years
 * calls it December. */
function SeasonCard({ data, cur }: { data: Season; cur: string }) {
  const months = data.months;
  // Scaled to the tallest thing drawn, so an unusually hot month is
  // never clipped by the shape it broke out of.
  const peak = Math.max(
    ...months.map((p) => Math.max(p.median, p.actual ?? 0)),
    1,
  );
  const height = (v: number) => `${Math.max(0, (v / peak) * 100)}%`;

  const off = data.pace == null ? null : data.pace - 1;
  const paceLabel =
    off == null
      ? null
      : Math.abs(off) < 0.005
        ? "running exactly to shape"
        : `running ${Math.round(Math.abs(off) * 100)}% ${off > 0 ? "above" : "below"} its usual pace`;

  return (
    <div className="report-card">
      <h2>The shape of a year</h2>
      <div className="sub2">
        median spend per calendar month
        {data.years && (
          <>
            , {data.years[0]}–{data.years[1]} ·{" "}
            {fmt(data.typical, cur, 0)} in a typical year
          </>
        )}
      </div>

      {months.length === 0 ? (
        <div className="empty">
          a whole year of history is needed before a shape appears —{" "}
          {data.elapsed === 0
            ? "nothing has finished yet"
            : `${fmt(data.ytd, cur, 0)} so far this year`}
        </div>
      ) : (
        <>
          {/* The one number this card exists to produce. */}
          <div className="sn-pace">
            {data.projected != null ? (
              <>
                <span className="num">{fmt(data.projected, cur, 0)}</span>
                <span>
                  {" "}
                  by the end of {data.year} · {fmt(data.ytd, cur, 0)}{" "}
                  through {data.elapsed} month
                  {data.elapsed === 1 ? "" : "s"}, {paceLabel}
                </span>
              </>
            ) : (
              <span className="muted">
                no month of {data.year} is over yet, so there is no pace to
                carry forward
              </span>
            )}
          </div>

          <div className="sn-chart">
            {months.map((p) => (
              <div
                key={p.month}
                className="sn-col"
                title={
                  `${calendarMonth(p.month)}: usually ${fmt(p.median, cur)}` +
                  ` (${p.samples} year${p.samples === 1 ? "" : "s"}, ` +
                  `${(p.share * 100).toFixed(1)}% of a year)` +
                  (p.actual == null
                    ? ""
                    : `\n${data.year}: ${fmt(p.actual, cur)}`)
                }
              >
                <span className="sn-stack">
                  <span
                    className="sn-med"
                    style={{ height: height(p.median) }}
                  />
                  {p.actual != null && (
                    <span
                      className={`sn-act ${p.actual > p.median ? "up" : "down"}`}
                      style={{ height: height(p.actual) }}
                    />
                  )}
                </span>
                <span className="sn-tick">
                  {calendarMonth(p.month).slice(0, 1)}
                </span>
              </div>
            ))}
          </div>

          <div className="fine">
            The wide bar is the median of {data.years?.[0]}–
            {data.years?.[1]}; the narrow one is {data.year}. At most five
            prior years feed it: the year being projected cannot also be
            the baseline it is measured against, and a longer reach is a
            different life at a different level rather than a better
            sample. The projection prices the months still ahead at what
            this year has been paying for the ones behind it.
          </div>
        </>
      )}
    </div>
  );
}


/** Who the money actually went to. The year card says groceries; this
 * says which supermarket, how often, and how much a visit costs — the
 * three numbers that decide whether a line is worth doing anything
 * about. */
function PayeeCard({ data, cur }: { data: Payees; cur: string }) {
  const max = data.items[0]?.spent ?? 0;
  const width = (v: number) => (max > 0 ? `${(v / max) * 100}%` : "0%");
  const pct = (v: number) => (v < 0.005 ? "<1%" : `${Math.round(v * 100)}%`);
  // How much of the year sits in the names below: the concentration
  // question, which is the one that says whether a list this short can
  // change anything.
  const covered = data.items.reduce((sum, p) => sum + p.share, 0);

  return (
    <div className="report-card">
      <h2>Where the money goes</h2>
      <div className="sub2">
        by payee, {windowLabel(data.window)} · {fmt(data.total, cur, 0)} in
        all
      </div>

      {data.items.length === 0 ? (
        <div className="empty">
          {data.anonymous > 0
            ? `no transaction in the window names a payee — ${fmt(data.anonymous, cur, 0)} of spend has nobody to rank`
            : "no spending in the window"}
        </div>
      ) : (
        <>
          <div className="ic-passive">
            <span className="num">{pct(covered)}</span>
            <span>
              {" "}
              of the year went to these {data.items.length} name
              {data.items.length === 1 ? "" : "s"}
              {data.anonymous > 0 && (
                <>
                  {" · "}
                  <span
                    title={`${data.anonymous_count} transactions carry no payee. They are in the total, so they are in the denominator of every share above — they are simply spend this card has no name to rank.`}
                  >
                    {pct(data.anonymous / data.total)} of it (
                    {fmt(data.anonymous, cur, 0)}) names nobody
                  </span>
                </>
              )}
            </span>
          </div>

          <div className="py-rows">
            {data.items.map((p) => (
              <div
                key={p.name}
                className="py-row"
                title={
                  `${p.name}: ${fmt(p.spent, cur)} over ${p.count} charge` +
                  `${p.count === 1 ? "" : "s"}, averaging ${fmt(p.average, cur)}` +
                  `\n${p.months} month${p.months === 1 ? "" : "s"}, ` +
                  `${p.categories} categor${p.categories === 1 ? "y" : "ies"}` +
                  ` · ${spanLabel(p.first, p.last)}`
                }
              >
                <span className="py-name">{p.name}</span>
                <span className="py-meta num">
                  {p.count}× {fmtCompact(p.average, cur)}
                </span>
                <span className="ic-track">
                  <span className="ic-bar" style={{ width: width(p.spent) }} />
                </span>
                <span className="py-amt num">{fmtCompact(p.spent, cur)}</span>
                <span className="py-pct num">{pct(p.share)}</span>
              </div>
            ))}
          </div>

          {data.others > 0 && (
            <div className="pj-note">
              {data.others} more name{data.others === 1 ? "" : "s"} below the
              cut, {fmt(data.others_spent, cur, 0)} between them
            </div>
          )}
        </>
      )}

      <div className="fine">
        Names are taken as the ledger writes them — deciding that two
        spellings are one merchant is a guess, and a ranking built on
        guesses ranks the guesses. Hover a row for the months, the
        categories and the span behind it: a name reaching one category
        every month is a subscription, and one reaching twenty is a card
        rather than a shop.
      </div>
    </div>
  );
}


/** One reason to doubt the page, as the card draws it. */
interface Doubt {
  key: string;
  label: string;
  detail: string;
  /** What rests on it, or null when the whole point is that the number
   * is unknowable. */
  amount: number | null;
  /** `gone` is money missing from the totals outright; `soft` is money
   * that is present but resting on something unconfirmed. */
  tone: "gone" | "soft";
  title?: string;
}

/** The page's own footnotes: everything above is a conversion, a flag
 * or a category away from being wrong, and this says by how much. */
function TrustCard({
  data,
  cur,
}: {
  data: ReportsView;
  cur: string;
}) {
  const t: Trust = data.trust;
  const doubts: Doubt[] = [];
  const plural = (n: number, one: string, many = `${one}s`) =>
    `${n} ${n === 1 ? one : many}`;

  if (data.unpriced.length > 0) {
    doubts.push({
      key: "unpriced",
      label: "No price",
      detail: listOf(data.unpriced),
      amount: null,
      tone: "gone",
      title: "Every amount in these is missing from every figure above.",
    });
  }
  if (t.stale.length > 0) {
    const head = t.stale[0]!;
    const rest = t.stale.length - 1;
    doubts.push({
      key: "stale",
      label: "Stale price",
      detail: `${head.commodity} last priced ${head.days} days ago${
        rest > 0 ? `, ${plural(rest, "other")}` : ""
      }`,
      amount: t.stale.reduce((sum, s) => sum + s.value, 0),
      tone: "soft",
      title: t.stale
        .map((s) => `${s.commodity}: ${s.last} · ${fmt(s.value, cur)}`)
        .join("\n"),
    });
  }
  if (t.flagged.total > 0) {
    doubts.push({
      key: "flagged",
      label: "Unconfirmed",
      detail:
        t.flagged.window > 0
          ? `${t.flagged.window} of ${plural(t.flagged.total, "flagged transaction")} land in the window`
          : `${plural(t.flagged.total, "flagged transaction")}, all older than the window`,
      amount: t.flagged.window > 0 ? t.flagged.amount : null,
      tone: "soft",
      title: t.flagged.recent
        .map(
          (f) =>
            `${f.date} · ${f.payee ?? f.narration ?? "—"} · ${fmt(f.amount, cur)}`,
        )
        .join("\n"),
    });
  }
  if (t.uncategorized.total > 0) {
    doubts.push({
      key: "uncategorized",
      label: "Uncategorized",
      detail: `${listOf(t.uncategorized.accounts)} · ${
        t.uncategorized.share != null
          ? `${(t.uncategorized.share * 100).toFixed(1)}% of the year's spend`
          : "of the year's spend"
      }`,
      amount: t.uncategorized.total,
      tone: "soft",
    });
  }
  if (t.warnings.length > 0) {
    doubts.push({
      key: "warnings",
      label: "Loader",
      detail: t.warnings[0]!,
      amount: null,
      tone: "gone",
      title: t.warnings.join("\n"),
    });
  }

  return (
    <div className="report-card">
      <h2>What these numbers rest on</h2>
      <div className="sub2">
        every figure above is one conversion away from wrong ·{" "}
        {windowLabel(t.window)}
      </div>

      {doubts.length === 0 ? (
        <div className="empty">
          nothing priced late, nothing flagged, nothing uncategorized
        </div>
      ) : (
        <div className="tr-rows">
          {doubts.map((d) => (
            <div key={d.key} className="tr-row" title={d.title}>
              <span className={`tr-dot ${d.tone}`} />
              <span className="tr-label">{d.label}</span>
              <span className="tr-detail">{d.detail}</span>
              <span className="tr-amt num">
                {d.amount == null ? "\u2014" : fmtCompact(d.amount, cur)}
              </span>
            </div>
          ))}
        </div>
      )}

      <div className="fine">
        A commodity with no price is missing from the totals; one priced
        long ago is present at a price that old, which is worse, because
        it still looks like a number. The amounts say how much of the page
        each doubt is holding up.
      </div>
    </div>
  );
}

export function Reports({ data, cur }: { data: ReportsView; cur: string }) {
  const f = data.fire;
  const hasTarget = f.fire_number > 0;
  const fill = Math.max(0, Math.min(1, f.progress ?? 0)) * 100;
  const cash = data.cashflow.slice(-24);
  const savings = savingsSeries(data.cashflow).slice(-24);

  return (
    <section id="reports">
      {data.unpriced.length > 0 && (
        <div className="notice">
          <b>{listOf(data.unpriced)}</b>{" "}
          {data.unpriced.length === 1 ? "has" : "have"} no price in {cur}, so
          every amount in {data.unpriced.length === 1 ? "it" : "them"} is
          missing from these numbers — the balances you hold and the income and
          spending that passed through {data.unpriced.length === 1 ? "it" : "them"}
          . Add <code>price</code> directives to bring{" "}
          {data.unpriced.length === 1 ? "it" : "them"} in.
        </div>
      )}
      <div className="report-card">
        <div className="fire-top">
          <div className="fire-main">
            <div className="lbl">Financial independence · 4% rule</div>
            <div className="fire-num num">
              {hasTarget ? fmt(f.fire_number, cur, 0) : "—"}
            </div>
            <div className="fire-sub">
              {hasTarget
                ? `25 × your annual spend of ${fmt(f.annual_spend, cur, 0)} (avg of ${windowLabel(f.window)})`
                : "not enough spending history to size a target yet"}
            </div>
          </div>
          <div className="fire-stats">
            <div className="fire-stat">
              <div className="lbl">Net worth</div>
              <div className="v num" title={fmt(f.net_worth, cur)}>
                {fmt(f.net_worth, cur, 0)}
              </div>
            </div>
            <div className="fire-stat">
              <div className="lbl">Spend / month</div>
              <div className="v num">{fmt(f.monthly_spend, cur)}</div>
            </div>
            <div className="fire-stat">
              <div className="lbl">Saved / month</div>
              <div
                className={`v num ${f.monthly_savings >= 0 ? "ok" : "bad"}`}
              >
                {fmt(f.monthly_savings, cur)}
              </div>
            </div>
            <div className="fire-stat">
              <div className="lbl">Runway</div>
              <div
                className="v num"
                title={`${fmt(data.runway.liquid, cur)} in cash, against ${fmt(f.monthly_spend, cur)} a month`}
              >
                {data.runway.months != null
                  ? `${data.runway.months.toFixed(1)} mo`
                  : "—"}
              </div>
              {data.runway.lean_months != null && (
                <div className="hint">
                  {data.runway.lean_months.toFixed(1)} on fixed costs alone
                </div>
              )}
            </div>
            <div className="fire-stat">
              <div className="lbl">4% pays today</div>
              <div className="v num">{fmt(f.swr_monthly, cur)} / mo</div>
            </div>
          </div>
        </div>
        {hasTarget && (
          <>
            <div className="fire-progress">
              <div className="fire-bar">
                <div className="fill" style={{ width: `${fill}%` }} />
              </div>
              <div className="fire-legend num">
                <span>
                  <b>{fmt(f.net_worth, cur, 0)}</b> today
                </span>
                <span>
                  {f.progress != null
                    ? `${(f.progress * 100).toFixed(1)}% of the way`
                    : "—"}
                </span>
                <span>
                  <b>{fmt(f.fire_number, cur, 0)}</b> target
                </span>
              </div>
            </div>
            {f.lean_number != null && (
              <div className="fire-lean">
                <b className="num">{fmt(f.lean_number, cur, 0)}</b> covers the
                charges that come back rather than the whole lifestyle
                {f.lean_progress != null
                  ? ` — ${(f.lean_progress * 100).toFixed(1)}% of the way there`
                  : ""}
                {data.recurring.coverage != null
                  ? `, on the ${
                      data.recurring.coverage < 0.005
                        ? "<1"
                        : Math.round(data.recurring.coverage * 100)
                    }% of spending that reads as recurring`
                  : ""}
                .
              </div>
            )}
            <div className="fire-scenarios">
              {f.scenarios.map((s, i) => (
                <div key={s.rate} className="scenario">
                  <div className="r">{Math.round(s.rate * 100)}% real return</div>
                  {s.months == null ? (
                    <>
                      <div className="t">not on this path</div>
                      <div className="eta">
                        current savings never reach the target
                      </div>
                    </>
                  ) : s.months === 0 ? (
                    <>
                      <div className="t">already there</div>
                      <div className="eta">the stash covers 4% today</div>
                    </>
                  ) : (
                    <>
                      <div className="t num">{duration(s.months)}</div>
                      <div className="eta num">
                        around {monthYear(addMonths(data.month, s.months))}
                      </div>
                    </>
                  )}
                  <div className="coast">
                    coasting: {coastLabel(f.coast[i]?.months)}
                  </div>
                </div>
              ))}
            </div>
            {f.steps.length > 0 && (
              <div className="fire-steps">
                <span className="lbl">
                  Saving more, at {Math.round((f.scenarios[1]?.rate ?? 0) * 100)}
                  %
                </span>
                {f.steps.map((s) => (
                  <span key={s.extra} className="step num">
                    +{fmt(s.extra, cur, 0)}/mo →{" "}
                    {s.months == null ? "still never" : duration(s.months)}
                  </span>
                ))}
              </div>
            )}
          </>
        )}
        <div className="fine">
          Net worth counts only balances convertible to {cur}; commodities
          without a price are left out. Flows include every account, even ones
          hidden from the budget page.
        </div>
      </div>

      <div className="report-grid">
        <div className="report-col">
          <div className="report-card">
            <h2>Net worth</h2>
            <div className="sub2">
              assets + liabilities at month end, converted to {cur}
            </div>
            <NetWorthChart points={data.net_worth} cur={cur} />
          </div>

          <GrowthCard growth={data.growth} cur={cur} />

          <div className="report-card">
            <h2>Monthly cashflow</h2>
            <div className="sub2">
              income − expenses
              {cash.length < data.cashflow.length
                ? ` · last ${cash.length} months`
                : ""}
            </div>
            <CashflowChart points={cash} cur={cur} />
          </div>

          <IncomeCard data={data.income} cur={cur} />

          <PayeeCard data={data.payees} cur={cur} />

          <ProjectsCard data={data.projects} cur={cur} />
        </div>

        <div className="report-col">
          <div className="report-card">
            <h2>Savings rate</h2>
            <div className="sub2">
              share of income kept · line = 3-mo trend
              {savings.length < data.cashflow.length
                ? ` · last ${savings.length} months`
                : ""}
            </div>
            <SavingsRateChart points={savings} cur={cur} />
          </div>

          <YearCard data={data} cur={cur} />

          <SeasonCard data={data.season} cur={cur} />

          <MoversCard data={data.movers} cur={cur} />

          <TrustCard data={data} cur={cur} />
        </div>
      </div>
    </section>
  );
}
