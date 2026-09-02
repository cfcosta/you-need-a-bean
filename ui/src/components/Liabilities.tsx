import type { ReactNode } from "react";
import { useState } from "react";

import type {
  Debt,
  DebtNotice,
  DebtPayment,
  DebtPoint,
  LiabilitiesView,
  TrailPoint,
} from "../api";
import { bounds, ticks } from "../chart";
import type { Amortization } from "../debt";
import {
  amortize,
  dayLabel,
  daysBetween,
  noticeText,
  ordinal,
  sliderRange,
} from "../debt";
import {
  duration,
  fmt,
  fmtCode,
  fmtCompact,
  monthName,
  monthShort,
  monthYear,
  ratio,
  windowLabel,
} from "../format";
import { addMonths } from "../months";
import { CardHead, Key } from "./Card";
import { Unpriced } from "./Unpriced";

/** Below this a balance is the dust of a conversion, not a debt. */
const EPS = 0.005;

/** The stack's colours start again after this many debts. */
const HUES = 5;

const pct = (v: number) =>
  `${Math.min(100, Math.max(0, v * 100)).toFixed(2)}%`;

function Stat({
  lbl,
  v,
  hint,
  tone,
  title,
}: {
  lbl: string;
  v: ReactNode;
  hint?: ReactNode;
  tone?: "ok" | "bad";
  title?: string;
}) {
  return (
    <div className="fire-stat">
      <div className="lbl">{lbl}</div>
      <div className={`v num${tone != null ? ` ${tone}` : ""}`} title={title}>
        {v}
      </div>
      {hint != null && <div className="hint">{hint}</div>}
    </div>
  );
}

/** The page's masthead: what is owed, what it costs, and when it ends. */
function Masthead({ data, cur }: { data: LiabilitiesView; cur: string }) {
  const i = data.interest;
  const owing = data.debts.filter((d) => d.owed > EPS);
  const loans = owing.filter((d) => d.kind === "installment");
  const total = owing.reduce((sum, d) => sum + d.owed, 0);
  const horizon = Math.max(0, ...loans.map((d) => d.payoff?.months ?? 0));
  const cover = data.cover;
  const hue = (k: number) => `d${(k % HUES) + 1}`;

  const free =
    data.debt_free != null
      ? monthYear(data.debt_free)
      : loans.length > 0
        ? "not at this pace"
        : owing.length > 0
          ? "cards aside"
          : "today";
  const freeHint =
    data.debt_free != null
      ? `${duration(horizon)} at today's payments`
      : loans.length > 0
        ? "a payment is not beating its interest"
        : "no loan to pay down";

  return (
    <div className="report-card fire debt">
      <div className="fire-top">
        <div className="fire-main">
          <div className="lbl">Owed · {monthName(data.month)}</div>
          <div className="fire-num num">{fmt(data.owed, cur, 0)}</div>
          <div
            className="fire-sub num"
            title={
              `Every Liabilities account with a balance, converted to ${cur}. ` +
              "Loans are paid down on a schedule; cards are charged and cleared."
            }
          >
            {data.debts.length === 0 ? (
              "no liability account carries a balance"
            ) : (
              <>
                <b>{fmtCompact(data.installment, cur)}</b> in loans ·{" "}
                <b>{fmtCompact(data.revolving, cur)}</b> on cards
              </>
            )}
          </div>
        </div>
        <div className="fire-stats">
          <Stat
            lbl="Interest this month"
            v={fmt(i.month, cur)}
            hint={`${fmt(i.year, cur, 0)} over ${windowLabel(i.window)}`}
            title="Every posting to an interest expense in a transaction that touched a debt."
          />
          <Stat
            lbl="A year at these rates"
            v={fmt(data.cost_year, cur, 0)}
            hint={
              data.blended_rate != null
                ? `${ratio(data.blended_rate)} across everything owed`
                : "no interest seen yet"
            }
            title="Each balance times the rate the ledger has been charging on it, added up."
          />
          <Stat lbl="Debt-free" v={free} hint={freeHint} />
          <Stat
            lbl="Cash against the cards"
            v={fmt(cover.cash, cur, 0)}
            tone={
              cover.owed > EPS ? (cover.covered ? "ok" : "bad") : undefined
            }
            hint={
              cover.owed <= EPS
                ? "nothing on the cards"
                : cover.covered
                  ? `${fmt(cover.after, cur, 0)} left after clearing them`
                  : `${fmt(-cover.after, cur, 0)} short of clearing them`
            }
            title={`${fmt(cover.cash, cur)} in budget accounts, against ${fmt(cover.owed, cur)} on the cards.`}
          />
          <Stat
            lbl="Interest earned"
            v={fmt(i.earned_month, cur)}
            hint={`${fmt(i.earned_year, cur, 0)} over ${windowLabel(i.window)}`}
            title="What the savings paid, for the figure paid to stand beside."
          />
        </div>
      </div>
      {owing.length > 0 && (
        <div className="debt-stack">
          <div className="stack-bar" role="img" aria-label="What is owed, by debt">
            {owing.map((d, k) => (
              <span
                key={d.account}
                className={`seg ${hue(k)}`}
                style={{ width: pct(d.owed / total) }}
                title={`${d.label} · ${fmt(d.owed, cur)}`}
              />
            ))}
          </div>
          <div className="key">
            {owing.map((d, k) => (
              <span key={d.account} className="key-item">
                <i className={`sw ${hue(k)}`} />
                {d.label} <b>{fmtCompact(d.owed, cur)}</b>
              </span>
            ))}
          </div>
        </div>
      )}
      {data.upcoming.length > 0 && (
        <div className="fire-steps">
          <span className="lbl">Coming up</span>
          {data.upcoming.map((u) => (
            <span key={`${u.account}${u.date}`} className="step num">
              {dayLabel(u.date)} · {u.label} · {fmt(u.amount, cur, 0)}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function NoticeRow({ n, cur }: { n: DebtNotice; cur: string }) {
  const tone = n.kind === "growing" ? " bad" : n.kind === "overpaid" ? " calm" : "";
  return (
    <div className={`notice${tone}`} role="status">
      <svg
        width="13"
        height="13"
        viewBox="0 0 16 16"
        fill="currentColor"
        aria-hidden="true"
      >
        {n.kind === "overpaid" ? (
          <path d="M8 1a7 7 0 1 1 0 14A7 7 0 0 1 8 1zm0 1.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11zM7.25 5h1.5v2.25H11v1.5H8.75V11h-1.5V8.75H5v-1.5h2.25V5z" />
        ) : (
          <path d="M8 1.4 15.2 14H.8L8 1.4zM7.25 6v4h1.5V6h-1.5zm0 5v1.5h1.5V11h-1.5z" />
        )}
      </svg>
      <span>{noticeText(n, cur)}</span>
    </div>
  );
}

function Fact({
  lbl,
  v,
  hint,
  tone,
  title,
}: {
  lbl: string;
  v: ReactNode;
  hint?: ReactNode;
  tone?: "ok" | "bad";
  title?: string;
}) {
  return (
    <div className="fact" title={title}>
      <div className="lbl">{lbl}</div>
      <div className={`v num${tone != null ? ` ${tone}` : ""}`}>{v}</div>
      {hint != null && <div className="hint">{hint}</div>}
    </div>
  );
}

/** The one line under a debt's name: what kind it is, what it costs,
 * and when it is paid. */
function spanOf(d: Debt, cur: string): string {
  const parts = [d.kind === "installment" ? "loan" : "card"];
  parts.push(d.rate != null ? `${ratio(d.rate)} a year` : "no interest seen");
  if (d.kind === "installment" && d.payment != null) {
    parts.push(
      d.due_day != null
        ? `${fmt(d.payment, cur, 0)} on the ${ordinal(d.due_day)}`
        : `${fmt(d.payment, cur, 0)} a month`,
    );
  } else if (d.due_day != null) {
    parts.push(`paid on the ${ordinal(d.due_day)}`);
  }
  return parts.join(" · ");
}

/** Where a loan has been and where its payments lead: the balance by
 * month so far, then the projection at today's payment, and beside it
 * the one at the payment the reader is trying. */
function PayoffChart({
  history,
  month,
  base,
  faster,
  cur,
}: {
  history: DebtPoint[];
  month: string;
  base: Amortization | null;
  faster: Amortization | null;
  cur: string;
}) {
  const W = 720;
  const H = 190;
  const TOP = 16;
  const BOT = 20;
  const PAD = 8;
  const past = history.flatMap((p) =>
    p.owed != null ? [{ month: p.month, owed: p.owed }] : [],
  );
  const proj = base?.curve ?? [];
  const fast = faster?.curve ?? [];
  const n = past.length + Math.max(proj.length, fast.length);
  const now = past[past.length - 1];
  if (now == null || n < 2) {
    return <div className="empty">nothing to chart yet</div>;
  }
  const k = past.length - 1;
  const [min, max] = bounds([...past.map((p) => p.owed), ...proj]);
  const x = (i: number) => PAD + (i * (W - 2 * PAD)) / (n - 1);
  const y = (v: number) => TOP + ((max - v) * (H - TOP - BOT)) / (max - min);
  const pt = (i: number, v: number) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`;
  const hist = past
    .map((p, i) => `${i === 0 ? "M" : "L"}${pt(i, p.owed)}`)
    .join("");
  const area = `${hist}L${pt(k, 0)}L${pt(0, 0)}Z`;
  const ahead = (curve: number[]) =>
    `M${pt(k, now.owed)}${curve.map((v, i) => `L${pt(k + 1 + i, v)}`).join("")}`;
  const months = [
    ...past.map((p) => p.month),
    ...Array.from({ length: n - past.length }, (_, i) =>
      addMonths(month, i + 1),
    ),
  ];
  const every = n > 48 ? 12 : n > 24 ? 6 : n > 12 ? 3 : 1;
  const labels = months.flatMap((m, i) =>
    i % every === 0
      ? [{ i, text: i === 0 || m.endsWith("-01") ? monthYear(m) : monthShort(m) }]
      : [],
  );
  const anchor = (px: number) =>
    px < PAD + 24 ? "start" : px > W - PAD - 24 ? "end" : "middle";
  const slot = (W - 2 * PAD) / (n - 1);
  const endX = (curve: number[]) => x(k + curve.length);
  // Two payoffs a few months apart would print on top of each other,
  // so the second flag hangs one line lower.
  const drop =
    fast.length > 0 && Math.abs(endX(fast) - endX(proj)) < 80 ? 13 : 0;
  const capX = k < n - 1 ? x(k) + 6 : x(k);
  const hover = (i: number) => {
    const m = monthName(months[i] ?? "");
    if (i <= k) return `${m}\nowed ${fmt(past[i]?.owed ?? 0, cur)}`;
    const j = i - k - 1;
    const at = (curve: number[]) =>
      curve[j] != null ? fmt(curve[j] ?? 0, cur) : "paid off";
    const more = fast.length > 0 ? `\npaying more: ${at(fast)}` : "";
    return `${m}\nat today's payment: ${at(proj)}${more}`;
  };

  return (
    <svg
      className="chart-svg"
      viewBox={`0 0 ${W} ${H}`}
      role="img"
      aria-label={`Balance by month, converted to ${cur}, and where the payments lead`}
    >
      {ticks(min, max, 4)
        .filter((v) => Math.abs(v) > 1e-9)
        .map((v) => (
          <g key={v}>
            <line x1={PAD} x2={W - PAD} y1={y(v)} y2={y(v)} className="grid" />
            <text x={PAD} y={y(v) - 3} className="axis">
              {fmtCompact(v, cur)}
            </text>
          </g>
        ))}
      <path d={area} className="debt-area" />
      <line x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} className="grid zero" />
      {proj.length > 0 && (
        <line
          x1={endX(proj)}
          x2={endX(proj)}
          y1={TOP - 2}
          y2={y(0)}
          className="grid end"
        />
      )}
      {fast.length > 0 && (
        <line
          x1={endX(fast)}
          x2={endX(fast)}
          y1={TOP - 2 + drop}
          y2={y(0)}
          className="grid end fast"
        />
      )}
      {proj.length > 0 && <path d={ahead(proj)} className="debt-proj" />}
      {fast.length > 0 && <path d={ahead(fast)} className="debt-fast" />}
      <path d={hist} className="debt-line" />
      {k < n - 1 && (
        <line x1={x(k)} x2={x(k)} y1={TOP - 6} y2={H - BOT} className="grid now" />
      )}
      <circle cx={x(k)} cy={y(now.owed)} r="3" className="debt-dot" />
      <text
        x={capX}
        y={y(now.owed) - 8}
        textAnchor={k < n - 1 ? "start" : "end"}
        className="cap"
      >
        {fmtCompact(now.owed, cur)}
      </text>
      {proj.length > 0 && (
        <text
          x={endX(proj) - 4}
          y={TOP - 5}
          textAnchor="end"
          className="cap owed"
        >
          {monthYear(addMonths(month, proj.length))}
        </text>
      )}
      {fast.length > 0 && (
        <text
          x={endX(fast) - 4}
          y={TOP - 5 + drop}
          textAnchor="end"
          className="cap fast"
        >
          {monthYear(addMonths(month, fast.length))}
        </text>
      )}
      {labels.map((l) => (
        <text
          key={l.i}
          x={x(l.i)}
          y={H - 5}
          textAnchor={anchor(x(l.i))}
          className="axis"
        >
          {l.text}
        </text>
      ))}
      {months.map((m, i) => (
        <rect
          key={m}
          x={x(i) - slot / 2}
          y={0}
          width={slot}
          height={H}
          fill="transparent"
        >
          <title>{hover(i)}</title>
        </rect>
      ))}
    </svg>
  );
}

/** A card's balance by day: up with every charge, down with every
 * payment, the sawtooth a card draws when it is cleared each month. */
function TrailChart({
  trail,
  today,
  cur,
}: {
  trail: TrailPoint[];
  today: string;
  cur: string;
}) {
  const W = 720;
  const H = 150;
  const TOP = 16;
  const BOT = 20;
  const PAD = 8;
  const from = trail[0]?.date;
  const pts = trail.flatMap((p) =>
    p.owed != null ? [{ date: p.date, owed: p.owed, delta: p.delta }] : [],
  );
  const first = pts[0];
  const last = pts[pts.length - 1];
  if (from == null || first == null || last == null) {
    return <div className="empty">nothing to chart yet</div>;
  }
  const span = Math.max(1, daysBetween(from, today));
  const x = (date: string) =>
    PAD +
    (Math.min(span, Math.max(0, daysBetween(from, date))) * (W - 2 * PAD)) /
      span;
  const [min, max] = bounds(pts.map((p) => p.owed));
  const y = (v: number) => TOP + ((max - v) * (H - TOP - BOT)) / (max - min);
  const pt = (px: number, v: number) => `${px.toFixed(1)},${y(v).toFixed(1)}`;
  let line = `M${pt(x(first.date), first.owed)}`;
  let prev = first.owed;
  for (const p of pts.slice(1)) {
    line += `L${pt(x(p.date), prev)}L${pt(x(p.date), p.owed)}`;
    prev = p.owed;
  }
  line += `L${pt(W - PAD, last.owed)}`;
  const area = `${line}L${pt(W - PAD, 0)}L${pt(x(first.date), 0)}Z`;
  const labels: { x: number; text: string }[] = [];
  for (let m = from.slice(0, 7); m <= today.slice(0, 7); m = addMonths(m, 1)) {
    const at = `${m}-01`;
    if (daysBetween(from, at) >= 0 && daysBetween(at, today) >= 0) {
      const px = x(at);
      if (px < W - PAD - 28) labels.push({ x: px, text: monthShort(m) });
    }
  }
  const hover = (p: (typeof pts)[number]) => {
    const moved =
      p.delta == null
        ? "at the start of the window"
        : p.delta > 0
          ? `+${fmt(p.delta, cur)} charged`
          : `${fmt(-p.delta, cur)} paid`;
    return `${dayLabel(p.date)}: ${moved}\nowed ${fmt(p.owed, cur)}`;
  };

  return (
    <svg
      className="chart-svg"
      viewBox={`0 0 ${W} ${H}`}
      role="img"
      aria-label={`Balance on the card by day, converted to ${cur}`}
    >
      {ticks(min, max, 2)
        .filter((v) => Math.abs(v) > 1e-9)
        .map((v) => (
          <g key={v}>
            <line x1={PAD} x2={W - PAD} y1={y(v)} y2={y(v)} className="grid" />
            <text x={PAD} y={y(v) - 3} className="axis">
              {fmtCompact(v, cur)}
            </text>
          </g>
        ))}
      <path d={area} className="debt-area" />
      <line x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} className="grid zero" />
      <path d={line} className="debt-line" />
      {pts
        .filter((p) => p.delta != null && p.delta < 0)
        .map((p) => (
          <circle
            key={`${p.date}${p.owed}`}
            cx={x(p.date)}
            cy={y(p.owed)}
            r="3"
            className="debt-pay"
          />
        ))}
      <circle cx={W - PAD} cy={y(last.owed)} r="3" className="debt-dot" />
      <text x={W - PAD} y={y(last.owed) - 8} textAnchor="end" className="cap">
        {fmtCompact(last.owed, cur)}
      </text>
      {labels.map((l) => (
        <text key={l.text} x={l.x + 3} y={H - 5} className="axis">
          {l.text}
        </text>
      ))}
      {pts.map((p, i) => (
        <circle key={i} cx={x(p.date)} cy={y(p.owed)} r="7" fill="transparent">
          <title>{hover(p)}</title>
        </circle>
      ))}
    </svg>
  );
}

function Payments({ rows, cur }: { rows: DebtPayment[]; cur: string }) {
  const top = Math.max(1e-9, ...rows.map((r) => r.total));
  return (
    <div className="pay-rows">
      <div className="lbl">Recent payments</div>
      {rows.map((r) => (
        <div
          key={r.date}
          className="pay-row num"
          title={`${fmt(r.principal, cur)} principal · ${fmt(r.interest, cur)} interest`}
        >
          <span className="d">{dayLabel(r.date)}</span>
          <span className="bar" aria-hidden="true">
            <i className="principal" style={{ width: pct(r.principal / top) }} />
            <i className="interest" style={{ width: pct(r.interest / top) }} />
          </span>
          <span className="t">{fmt(r.total, cur)}</span>
        </div>
      ))}
    </div>
  );
}

function LoanCard({
  debt: d,
  month,
  cur,
}: {
  debt: Debt;
  month: string;
  cur: string;
}) {
  const [extra, setExtra] = useState(0);
  const range = sliderRange(d.payment ?? 0);
  const base =
    d.payment != null ? amortize(d.owed, d.rate ?? 0, d.payment) : null;
  const faster =
    d.payment != null && extra > 0
      ? amortize(d.owed, d.rate ?? 0, d.payment + extra)
      : null;
  const paid = d.principal_paid + d.interest_paid;
  const keys = [
    { sw: "debt", label: "owed" },
    { sw: "proj", label: "at today's payment" },
  ];
  if (faster != null) {
    keys.push({ sw: "fast", label: `paying ${fmt(extra, cur, 0)} more` });
  }

  return (
    <div className="report-card debt-card">
      <CardHead
        title={d.label}
        span={spanOf(d, cur)}
        figure={fmt(d.owed, cur)}
        note={d.progress != null ? `${ratio(d.progress)} paid off` : "owed"}
      />
      <PayoffChart
        history={d.history}
        month={month}
        base={base}
        faster={faster}
        cur={cur}
      />
      <Key items={keys} />
      <div className="debt-facts">
        <Fact
          lbl="Next payment"
          v={d.next_due != null ? dayLabel(d.next_due) : "—"}
          hint={
            d.owed <= EPS
              ? "nothing to pay"
              : d.payment != null
                ? `${fmt(d.payment, cur)} usual`
                : "none seen yet"
          }
        />
        <Fact
          lbl="Paid off"
          v={d.payoff != null ? monthYear(d.payoff.month) : "—"}
          hint={
            d.payoff != null
              ? `${duration(d.payoff.months)} · ${fmt(d.payoff.interest, cur, 0)} more interest`
              : d.payment != null
                ? "not at this payment"
                : "no payment to project"
          }
        />
        <Fact
          lbl="Paid so far"
          v={fmt(paid, cur, 0)}
          hint={`from a peak of ${fmt(d.peak, cur, 0)}`}
        />
      </div>
      {paid > EPS && (
        <>
          <div className="split-bar" aria-hidden="true">
            <i className="principal" style={{ width: pct(d.principal_paid / paid) }} />
            <i className="interest" style={{ width: pct(d.interest_paid / paid) }} />
          </div>
          <Key
            items={[
              { sw: "principal", label: `principal ${fmtCompact(d.principal_paid, cur)}` },
              { sw: "interest", label: `interest ${fmtCompact(d.interest_paid, cur)}` },
            ]}
          />
        </>
      )}
      {d.payment != null && base != null && d.owed > EPS && (
        <div className="debt-slider">
          <label>
            <span className="lbl">Pay more each month</span>
            <input
              type="range"
              min={0}
              max={range.max}
              step={range.step}
              value={extra}
              onChange={(e) => setExtra(Number(e.currentTarget.value))}
              aria-valuetext={`${fmt(extra, cur, 0)} more a month`}
            />
          </label>
          <div className="slider-read num">
            {faster == null ? (
              <>
                at <b>{fmt(d.payment, cur, 0)}</b> a month, paid off{" "}
                {monthYear(addMonths(month, base.months))} with{" "}
                <b>{fmt(base.interest, cur, 0)}</b> more interest
              </>
            ) : (
              <>
                <b>+{fmt(extra, cur, 0)}</b> a month: paid off{" "}
                {monthYear(addMonths(month, faster.months))},{" "}
                <b>{duration(base.months - faster.months)}</b> sooner, and{" "}
                <b>{fmt(base.interest - faster.interest, cur, 0)}</b> less
                interest
              </>
            )}
          </div>
        </div>
      )}
      {d.payments.length > 0 && (
        <Payments rows={d.payments.slice(0, 6)} cur={cur} />
      )}
    </div>
  );
}

function RevolvingCard({
  debt: d,
  today,
  cur,
}: {
  debt: Debt;
  today: string;
  cur: string;
}) {
  const c = d.cycle;
  const balances = Object.entries(d.balances);
  const foreign = balances.some(([code]) => code !== cur);
  const note =
    c?.in_full === true
      ? "cleared last cycle"
      : c?.carried != null
        ? `carrying ${fmtCompact(c.carried, cur)}`
        : d.owed <= EPS
          ? "nothing owed"
          : "owed";

  return (
    <div className="report-card debt-card">
      <CardHead
        title={d.label}
        span={spanOf(d, cur)}
        figure={fmt(d.owed, cur)}
        note={note}
      />
      <TrailChart trail={d.trail} today={today} cur={cur} />
      <Key
        items={[
          { sw: "debt", label: "on the card" },
          { sw: "pay", label: "payment" },
        ]}
      />
      <div className="debt-facts">
        <Fact
          lbl="This month"
          v={fmt(c?.charges ?? 0, cur, 0)}
          hint={`charged · ${fmt(c?.payments ?? 0, cur, 0)} paid`}
        />
        <Fact
          lbl="Next payment"
          v={d.next_due != null ? dayLabel(d.next_due) : "—"}
          hint={
            d.owed <= EPS
              ? "nothing to pay"
              : d.payment != null
                ? `${fmt(d.payment, cur)} usual`
                : "none seen yet"
          }
        />
        <Fact
          lbl="Interest paid"
          v={fmt(d.interest_paid, cur, 0)}
          tone={d.interest_paid > EPS ? "bad" : undefined}
          hint={d.rate != null ? `${ratio(d.rate)} a year` : "none charged"}
        />
      </div>
      {foreign && (
        <div className="debt-balances mono">
          {balances.map(([code, v]) => (
            <span key={code}>{fmtCode(-v, code)}</span>
          ))}
        </div>
      )}
    </div>
  );
}

export function Liabilities({
  data,
  cur,
  today,
  basis,
}: {
  data: LiabilitiesView;
  cur: string;
  today: string;
  basis: number;
}) {
  const loans = data.debts.filter((d) => d.kind === "installment");
  const cards = data.debts.filter((d) => d.kind === "revolving");
  const one = loans.length === 0 || cards.length === 0;

  return (
    <section id="reports" className="debts">
      <Unpriced codes={data.unpriced} cur={cur} where="every figure here" />
      <Masthead data={data} cur={cur} />
      {data.notices.length > 0 && (
        <div className="notices">
          {data.notices.map((n) => (
            <NoticeRow key={`${n.kind}${n.account}`} n={n} cur={cur} />
          ))}
        </div>
      )}
      {data.debts.length === 0 ? (
        <div className="report-card debt-none">
          <h2>Nothing owed</h2>
          <p>
            No liability account holds a balance or has moved in the last{" "}
            {basis} months.
          </p>
        </div>
      ) : (
        <div className={`report-grid${one ? " one" : ""}`}>
          {loans.length > 0 && (
            <div className="report-col">
              {loans.map((d) => (
                <LoanCard key={d.account} debt={d} month={data.month} cur={cur} />
              ))}
            </div>
          )}
          {cards.length > 0 && (
            <div className="report-col">
              {cards.map((d) => (
                <RevolvingCard key={d.account} debt={d} today={today} cur={cur} />
              ))}
            </div>
          )}
        </div>
      )}
      {/* The fill under a balance: strongest at the line, fading to the
          baseline, so the band reads as a quantity owed. */}
      <svg className="defs" aria-hidden="true" focusable="false">
        <defs>
          <linearGradient id="g-owed" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" style={{ stopColor: "var(--over)" }} stopOpacity="0.45" />
            <stop offset="1" style={{ stopColor: "var(--over)" }} stopOpacity="0.03" />
          </linearGradient>
        </defs>
      </svg>
    </section>
  );
}
