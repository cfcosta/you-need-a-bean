import type { CashflowPoint, NetWorthPoint, ReportsView } from "../api";
import { fmt, fmtCompact, monthName, monthShort, windowLabel } from "../format";

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

const monthYear = (m: string) => `${monthShort(m)} ${m.slice(0, 4)}`;

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
  const [min, max] = bounds(points.map((p) => p.net));
  const x = (i: number) =>
    n === 1 ? W / 2 : PAD + (i * (W - 2 * PAD)) / (n - 1);
  const y = (v: number) => TOP + ((max - v) * (H - TOP - BOT)) / (max - min);
  const line = points
    .map((p, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(p.net).toFixed(1)}`)
    .join("");
  const area = `${line}L${x(n - 1).toFixed(1)},${y(0).toFixed(1)}L${x(0).toFixed(1)},${y(0).toFixed(1)}Z`;

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
      <line x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} className="grid zero" />
      <path d={area} className="nw-area" />
      <path d={line} className="nw-line" />
      <circle cx={x(n - 1)} cy={y(last.net)} r="3" className="nw-dot" />
      <text
        x={W - PAD}
        y={y(last.net) - 7}
        textAnchor="end"
        className="cap"
      >
        {fmtCompact(last.net, cur)}
      </text>
      {labels.map((l) => (
        <text key={l.i} x={x(l.i)} y={H - 5} textAnchor="middle" className="axis">
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
          <title>{`${monthName(p.month)}\nnet ${fmt(p.net, cur)}\nassets ${fmt(p.assets, cur)} · liabilities ${fmt(p.liabilities, cur)}`}</title>
        </rect>
      ))}
    </svg>
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

function YearCard({ data, cur }: { data: ReportsView; cur: string }) {
  const groups = data.year.groups;
  const total = groups.reduce((s, g) => s + g.total, 0);
  const top = groups.slice(0, 9);
  const rest = groups.slice(9);
  const restTotal = rest.reduce((s, g) => s + g.total, 0);
  const max = groups[0]?.total ?? 0;
  const width = (v: number) =>
    max > 0 ? `${Math.min(100, (v / max) * 100)}%` : "0%";
  const share = (v: number) => {
    const p = (v / total) * 100;
    return p >= 0.5 ? `${Math.round(p)}%` : "<1%";
  };

  return (
    <div className="report-card">
      <h2>Where the year went</h2>
      <div className="sub2">
        spend by group, {windowLabel(data.year.window)} ·{" "}
        {fmt(total, cur, 0)} total
      </div>
      {groups.length === 0 ? (
        <div className="empty">no spending in the last year</div>
      ) : (
        <div className="yr-rows">
          {top.map((g) => (
            <div
              key={g.name}
              className="yr-row"
              title={`${g.name}: ${fmt(g.total, cur)}`}
            >
              <span className="yr-name">{g.name}</span>
              <span className="yr-track">
                <span className="yr-bar" style={{ width: width(g.total) }} />
              </span>
              <span className="yr-amt num">{fmtCompact(g.total, cur)}</span>
              <span className="yr-pct num">{share(g.total)}</span>
            </div>
          ))}
          {rest.length > 0 && (
            <div
              className="yr-row other"
              title={rest.map((g) => `${g.name}: ${fmt(g.total, cur)}`).join("\n")}
            >
              <span className="yr-name">{rest.length} more groups</span>
              <span className="yr-track">
                <span className="yr-bar" style={{ width: width(restTotal) }} />
              </span>
              <span className="yr-amt num">{fmtCompact(restTotal, cur)}</span>
              <span className="yr-pct num">{share(restTotal)}</span>
            </div>
          )}
        </div>
      )}
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
            <div className="fire-scenarios">
              {f.scenarios.map((s) => (
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
                </div>
              ))}
            </div>
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
        </div>
      </div>
    </section>
  );
}
