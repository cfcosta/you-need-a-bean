import type { ReactNode, RefObject } from "react";
import { useEffect, useRef, useState } from "react";

import type {
  Beaten,
  Debt,
  DebtNotice,
  DebtPayment,
  DebtPoint,
  Extra,
  LiabilitiesView,
  Makeup,
  TrailPoint,
  Treadmill,
  Upcoming,
} from "../api";
import { bounds, ticks } from "../chart";
import type { Amortization, Order, Plan, PlanDebt } from "../debt";
import {
  addDays,
  amortize,
  attack,
  beatenText,
  columns,
  costShares,
  coverLine,
  dayLabel,
  daysBetween,
  dueEvents,
  earnedLine,
  emphasize,
  foreignText,
  hues,
  monthPlan,
  noticeText,
  payingDown,
  plan,
  rateTone,
  sliderRange,
  sliderStart,
  stripMarks,
  targets,
  treadmillFacts,
  verdict,
} from "../debt";
import {
  duration,
  fmt,
  fmtCompact,
  monthName,
  monthShort,
  monthYear,
  ratio,
  windowLabel,
} from "../format";
import { calendar } from "../ics";
import { addMonths } from "../months";
import { Key } from "./Card";
import { Unpriced } from "./Unpriced";

/** Below this a balance is the dust of a conversion, not a debt. */
const EPS = 0.005;

/** A share of a bar wide enough to print its figure inside. */
const LABELLED = 0.07;

const pct = (v: number) =>
  `${Math.min(100, Math.max(0, v * 100)).toFixed(2)}%`;

/** A name cut to fit a fixed column, with the cut shown. */
const cut = (s: string, n: number) =>
  s.length > n ? `${s.slice(0, n - 1).trimEnd()}…` : s;

/** The colour class of a debt, or the grey of one with nothing on it. */
const hueOf = (hue: Map<string, string>, account: string) =>
  hue.get(account) ?? "dn";

/** The width of a box, kept current, so a drawing can be laid out in
 * pixels instead of scaled up from a fixed canvas. */
function useWidth<T extends HTMLElement>(
  fallback = 720,
): [RefObject<T | null>, number] {
  const ref = useRef<T>(null);
  const [w, setW] = useState(fallback);
  useEffect(() => {
    const el = ref.current;
    if (el == null || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width;
      if (width != null && width > 0) setW(Math.round(width));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return [ref, w];
}

/** A sentence with its figures in bold, so a scan still finds them. */
function Prose({ text, className }: { text: string; className?: string }) {
  return (
    <p className={`debt-lede${className != null ? ` ${className}` : ""}`}>
      {emphasize(text).map((s, i) =>
        s.strong ? <b key={i}>{s.text}</b> : <span key={i}>{s.text}</span>,
      )}
    </p>
  );
}

/** A conclusion the page draws, as a call in a couple of words with
 * the reason beside it. */
/** Roughly how tall a debt's card comes out, in pixels, so a group
 * split across two columns ends both at about the same place. The
 * numbers are measured off the rendered page and each part is only
 * counted when the card actually draws it. They are an estimate: only
 * the ratios between them matter, and being out by a little moves at
 * most one card to the other column. */
function tall(d: Debt): number {
  const owing = d.owed > EPS;
  const carried = (d.cycle?.carried ?? 0) > EPS && owing;
  const rows = d.makeup?.rows.length ?? 0;
  const pays = Math.min(d.payments.length, 6);
  const head = 295 + (d.foreign.length > 0 ? 29 : 0);
  if (d.kind === "revolving") {
    return (
      head +
      (d.limit != null && d.utilisation != null ? 38 : 0) +
      (rows > 0 ? 55 + 39 * rows : 0) +
      (d.treadmill != null ? 275 : 0) +
      (carried && d.payment != null ? 71 : 0)
    );
  }
  return (
    head +
    35 +
    (d.principal_paid + d.interest_paid > EPS ? 55 : 0) +
    (d.collateral != null && owing ? 38 : 0) +
    (d.rate != null && owing ? 42 : 0) +
    (d.payment != null && owing ? 71 : 0) +
    (pays > 0 ? 26 + 25 * pays : 0)
  );
}

function Verdict({ call, why, tone }: { call: string; why: string; tone: string }) {
  return (
    <div className={`verdict ${tone}`}>
      <b>{call}</b>
      <span>{why}</span>
    </div>
  );
}

interface Read {
  v: string;
  l: string;
  tone?: "ok" | "bad";
}

/** A row of figures, each with what it is beside it: what a slider's
 * setting comes to, read at a glance. */
function Readout({ items }: { items: Read[] }) {
  return (
    <div className="readout num">
      {items.map((i) => (
        <span key={i.l} className={`ro${i.tone != null ? ` ${i.tone}` : ""}`}>
          <b>{i.v}</b> {i.l}
        </span>
      ))}
    </div>
  );
}

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

/** What is owed and what it costs, as two bars cut the same way, so a
 * debt that is a sliver of one and a slab of the other shows itself. */
function Shares({
  debts,
  hue,
  cur,
}: {
  debts: Debt[];
  hue: Map<string, string>;
  cur: string;
}) {
  const shares = costShares(debts);
  const costed = shares.some((s) => s.cost > 0);
  const owing = debts.filter((d) => d.owed > EPS);
  return (
    <div className="debt-shares">
      <div className="share-row">
        <span className="lbl">owed</span>
        <div className="stack-bar shares" role="img" aria-label="What is owed, by debt">
          {shares.map((s) => (
            <span
              key={s.account}
              className={`seg ${hueOf(hue, s.account)}`}
              style={{ width: pct(s.owed) }}
              title={`${s.label} · ${ratio(s.owed)} of what is owed`}
            >
              {s.owed >= LABELLED && <b>{ratio(s.owed)}</b>}
            </span>
          ))}
        </div>
      </div>
      {costed && (
        <div className="share-row">
          <span className="lbl">cost</span>
          <div
            className="stack-bar shares"
            role="img"
            aria-label="What the interest costs, by debt"
          >
            {shares
              .filter((s) => s.cost > 0)
              .map((s) => (
                <span
                  key={s.account}
                  className={`seg ${hueOf(hue, s.account)}`}
                  style={{ width: pct(s.cost) }}
                  title={`${s.label} · ${ratio(s.cost)} of the cost`}
                >
                  {s.cost >= LABELLED && <b>{ratio(s.cost)}</b>}
                </span>
              ))}
          </div>
        </div>
      )}
      <div className="key">
        {owing.map((d) => (
          <span key={d.account} className="key-item">
            <i className={`sw ${hueOf(hue, d.account)}`} />
            {d.label} <b>{fmtCompact(d.owed, cur)}</b>
            {d.rate != null && d.rate > EPS && (
              <span className="at">{ratio(d.rate)}</span>
            )}
          </span>
        ))}
      </div>
    </div>
  );
}

/** The next month as a strip of days, with every payment due on it
 * standing where it falls. */
function PayStrip({
  upcoming,
  today,
  hue,
  cur,
}: {
  upcoming: Upcoming[];
  today: string;
  hue: Map<string, string>;
  cur: string;
}) {
  const [ref, W] = useWidth<HTMLDivElement>();
  const PAD = 10;
  const DAYS = 31;
  // Two labels closer than this many pixels go in different lanes.
  const marks = stripMarks(upcoming, today, DAYS, Math.min(0.5, 170 / W));
  const lanes = Math.max(0, ...marks.map((m) => m.lane)) + 1;
  const BASE = 12 + 13 * lanes;
  const H = BASE + 18;
  const x = (v: number) => PAD + v * (W - 2 * PAD);
  const anchor = (px: number) =>
    px < 70 ? "start" : px > W - 70 ? "end" : "middle";
  const nudge = (px: number) => (px < 70 ? -4 : px > W - 70 ? 4 : 0);
  const weeks = [0, 7, 14, 21, 28];
  return (
    <div className="pay-strip" ref={ref}>
      <span className="lbl">Coming up</span>
      <svg
        className="chart-svg strip"
        width={W}
        height={H}
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label="Payments due in the next 31 days"
      >
        <line x1={PAD} x2={W - PAD} y1={BASE} y2={BASE} className="strip-line" />
        {Array.from({ length: DAYS + 1 }, (_, i) => (
          <line
            key={i}
            x1={x(i / DAYS)}
            x2={x(i / DAYS)}
            y1={BASE}
            y2={BASE + (i % 7 === 0 ? 6 : 3)}
            className="strip-tick"
          />
        ))}
        {weeks.map((i) => (
          <text
            key={i}
            x={x(i / DAYS)}
            y={BASE + 16}
            textAnchor={i === 0 ? "start" : "middle"}
            className="axis"
          >
            {i === 0 ? "today" : dayLabel(addDays(today, i))}
          </text>
        ))}
        {marks.map((m) => {
          const px = x(m.x);
          const ly = BASE - 13 - m.lane * 13;
          return (
            <g key={`${m.account}${m.date}`} className="strip-mark">
              <title>{`${dayLabel(m.date)} · ${m.label} · ${fmt(m.amount, cur)}`}</title>
              {m.lane > 0 && (
                <line x1={px} x2={px} y1={ly + 3} y2={BASE - 6} className="strip-stem" />
              )}
              <circle cx={px} cy={BASE} r="4.5" className={`mark ${hueOf(hue, m.account)}`} />
              <text x={px + nudge(px)} y={ly} textAnchor={anchor(px)} className="strip-label">
                {m.label} <tspan className="amt">{fmt(m.amount, cur, 0)}</tspan>
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}

/** The page's masthead: what is owed, what that means, and when it
 * ends. */
function Masthead({
  data,
  hue,
  today,
  cur,
}: {
  data: LiabilitiesView;
  hue: Map<string, string>;
  today: string;
  cur: string;
}) {
  const i = data.interest;
  const owing = data.debts.filter((d) => d.owed > EPS);
  const down = payingDown(data.debts);
  const horizon = Math.max(
    0,
    ...data.debts
      .filter((d) => down.some((p) => p.account === d.account))
      .map((d) => d.payoff?.months ?? 0),
  );
  const cover = data.cover;

  const free =
    data.debt_free != null
      ? monthYear(data.debt_free)
      : down.length > 0
        ? "not at this pace"
        : owing.length > 0
          ? "cards aside"
          : "today";
  // With every payment rolling on to the next debt as one ends, the
  // last one goes sooner than the schedules say on their own.
  const rolled = data.debt_free != null ? plan(down, 0, "avalanche") : null;
  const sooner =
    rolled != null && data.debt_free != null
      ? addMonths(data.month, rolled.months)
      : null;
  const freeHint =
    data.debt_free != null
      ? sooner != null && sooner < data.debt_free
        ? `${monthYear(sooner)} with payments rolling on`
        : `${duration(horizon)} at today's payments`
      : down.length > 0
        ? "a payment is not beating its interest"
        : "nothing is being paid down";
  const coverText = coverLine(cover, cur);
  const earnedText = earnedLine(i, cur);

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
            lbl="Costs a month"
            v={fmt(data.cost_year / 12, cur, 0)}
            hint={
              data.blended_rate != null
                ? `${fmt(data.cost_year, cur, 0)} a year at ${ratio(data.blended_rate)}`
                : "no interest seen yet"
            }
            title="Each balance times the rate the ledger has been charging on it, added up and spread over the year. The rate is the blend across everything owed."
          />
          <Stat
            lbl="Debt-free"
            v={free}
            hint={freeHint}
            title={
              sooner != null && data.debt_free != null && sooner < data.debt_free
                ? `${monthYear(data.debt_free)} on today's schedules; ${monthYear(sooner)} if each payment rolls on to the next debt when its own ends.`
                : undefined
            }
          />
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
            title={coverText ?? undefined}
          />
          <Stat
            lbl={`Interest paid · ${windowLabel(i.window)}`}
            v={fmt(i.year, cur, 0)}
            tone={
              i.year > EPS || i.earned_year > EPS
                ? i.earned_year >= i.year
                  ? "ok"
                  : "bad"
                : undefined
            }
            hint={
              i.earned_year > EPS
                ? `against ${fmt(i.earned_year, cur, 0)} earned in savings`
                : "and nothing earned in savings"
            }
            title={earnedText ?? undefined}
          />
        </div>
      </div>
      {owing.length > 0 && <Shares debts={data.debts} hue={hue} cur={cur} />}
      {data.upcoming.length > 0 && (
        <PayStrip upcoming={data.upcoming} today={today} hue={hue} cur={cur} />
      )}
    </div>
  );
}

/** Which debt the spare goes to first. */
function OrderToggle({
  order,
  setOrder,
}: {
  order: Order;
  setOrder: (o: Order) => void;
}) {
  return (
    <div className="seg" role="group" aria-label="Which debt the spare goes to first">
      {ORDERS.map((o) => (
        <button
          key={o.order}
          type="button"
          className={o.order === order ? "on" : ""}
          title={o.hint}
          onClick={() => setOrder(o.order)}
        >
          {o.title}
        </button>
      ))}
    </div>
  );
}

/** The cash in budget accounts cut into what it is spoken for by and
 * what is spare, so the spare figure shows where it came from. */
/** The cash in budget accounts cut into what it is spoken for by and
 * what is spare; the spare cut again into what this month's plan
 * sends to the debts and what is left over. */
function RoomBar({
  x,
  placed,
  left,
  note,
  cur,
}: {
  x: Extra;
  /** What the plan puts on the debts, out of the spare. */
  placed: number;
  /** The spare no debt takes. */
  left: number;
  /** Why some of it is left, when there is a reason. */
  note?: string;
  cur: string;
}) {
  if (x.cash <= EPS) return null;
  const parts = [
    { sw: "due", v: x.due, label: "due in 31 days" },
    { sw: "spend", v: x.spend, label: "a month of spending" },
    { sw: "buffer", v: x.buffer, label: "fixed costs kept back" },
    ...(placed > EPS
      ? [
          { sw: "sent", v: placed, label: "to the debts" },
          { sw: "spare", v: left, label: "left over", title: note },
        ]
      : [{ sw: "spare", v: x.now, label: "spare", title: note }]),
  ];
  const segs = parts.filter((p) => p.v > EPS);
  const base = Math.max(
    x.cash,
    segs.reduce((s, p) => s + p.v, 0),
    EPS,
  );
  return (
    <div className="room">
      <div
        className="stack-bar room-bar"
        role="img"
        aria-label={`The ${fmt(x.cash, cur, 0)} in budget accounts, and what it is spoken for by`}
      >
        {segs.map((p) => (
          <span
            key={p.sw}
            className={`seg ${p.sw}`}
            style={{ width: pct(p.v / base) }}
            title={`${p.label} · ${fmt(p.v, cur)}`}
          />
        ))}
      </div>
      <Key
        items={parts
          .filter((p) => p.v > EPS || p.sw === "spare")
          .map((p) => ({
            sw: p.sw,
            title: p.title,
            label: (
              <>
                {p.label} <b>{fmtCompact(p.v, cur)}</b>
              </>
            ),
          }))}
      />
    </div>
  );
}

/** This month's payments, with the spare placed on top of them: what
 * to send to each debt, on what day, and what is left on it after.
 * The spare is worked out from the cash, not asked for. */
function PlanCard({
  data,
  hue,
  today,
  cur,
}: {
  data: LiabilitiesView;
  hue: Map<string, string>;
  today: string;
  cur: string;
}) {
  const [order, setOrder] = useState<Order>("avalanche");
  const x = data.extra;
  const assumed = data.assumed_return;
  const { rows, left } = monthPlan(
    data.debts,
    data.upcoming,
    x.now,
    order,
    assumed,
    today,
  );
  const inLine = targets(data.debts, order, assumed);
  const skipped = payingDown(data.debts).length - inLine.length;
  const sent = rows.reduce((s, r) => s + r.extra, 0);
  const total = rows.reduce((s, r) => s + r.usual + r.extra, 0);
  const note =
    left > EPS && skipped > 0
      ? `${skipped === 1 ? "A loan" : `${skipped} loans`} under the ${ratio(assumed)} a portfolio is assumed to make ${skipped === 1 ? "is" : "are"} skipped: this does better invested.`
      : undefined;
  const download = () => {
    const text = calendar(dueEvents(data.upcoming, data.debts, cur), today);
    const url = URL.createObjectURL(
      new Blob([text], { type: "text/calendar;charset=utf-8" }),
    );
    const a = document.createElement("a");
    a.href = url;
    a.download = "payments.ics";
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };

  return (
    <div className="report-card plan-card">
      <div className="card-head">
        <h2>This month</h2>
        <div className="card-fig">
          <span className="card-total num">{fmt(total, cur, 0)}</span>
          <span className="card-note">
            {rows.length === 0
              ? "nothing to send"
              : rows.length === 1
                ? "one payment to send"
                : `${rows.length} payments to send`}
          </span>
        </div>
      </div>
      <RoomBar x={x} placed={sent} left={left} note={note} cur={cur} />
      {rows.length > 0 && (
        <div className="plan-rows">
          <div className="plan-bar">
            <span className="lbl">Send</span>
            {x.now > EPS && inLine.length > 1 && (
              <OrderToggle order={order} setOrder={setOrder} />
            )}
          </div>
          {rows.map((r) => (
            <div
              key={r.account}
              className={`plan-row num ${hueOf(hue, r.account)}`}
            >
              <span className="d">{dayLabel(r.date)}</span>
              <i className="dot" aria-hidden="true" />
              <span className="who">{r.label}</span>
              <span
                className="amt"
                title={
                  r.extra > EPS && r.usual > EPS
                    ? `${fmt(r.usual, cur)} usual + ${fmt(r.extra, cur)} extra`
                    : undefined
                }
              >
                {fmt(r.usual + r.extra, cur)}
                {r.extra > EPS && (
                  <span className="plus">
                    {r.usual > EPS
                      ? `+ ${fmt(r.extra, cur, 0)} extra`
                      : "extra, any day"}
                  </span>
                )}
              </span>
              <span className="after">
                {r.after > EPS ? `${fmt(r.after, cur)} left` : "cleared"}
              </span>
            </div>
          ))}
        </div>
      )}
      {data.upcoming.length > 0 && (
        <div className="plan-foot">
          <button
            type="button"
            className="plan-btn"
            onClick={download}
            title="An .ics file: one repeating event per debt, on its due day."
          >
            <svg
              width="13"
              height="13"
              viewBox="0 0 16 16"
              fill="currentColor"
              aria-hidden="true"
            >
              <path d="M3 2h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1zm.5 4v7h9V6h-9zM5 1h1.5v2H5V1zm4.5 0H11v2H9.5V1zM5 8h2v2H5V8zm3 0h2v2H8V8z" />
            </svg>
            Add the due dates to your calendar
          </button>
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

/** A share drawn as a ring, in the debt's colour, with the figure in
 * the middle. */
function Ring({
  value,
  hue,
  text,
  label,
}: {
  value: number;
  hue: string;
  text: string;
  label: string;
}) {
  const r = 17;
  const c = 2 * Math.PI * r;
  const v = Math.min(1, Math.max(0, value));
  return (
    <svg
      className={`debt-ring ${hue}`}
      width="44"
      height="44"
      viewBox="0 0 44 44"
      role="img"
      aria-label={label}
      style={{ ["--dash" as string]: (c * v).toFixed(2) }}
    >
      <circle cx="22" cy="22" r={r} className="track" />
      {v > 0 && (
        <circle
          cx="22"
          cy="22"
          r={r}
          className="fill"
          strokeDasharray={`${(c * v).toFixed(2)} ${c.toFixed(2)}`}
          transform="rotate(-90 22 22)"
        />
      )}
      <text x="22" y="22" textAnchor="middle" dominantBaseline="central" className="num">
        {text}
      </text>
    </svg>
  );
}

/** A share of something drawn as a bar, with what it is a share of
 * written at either end. */
function Gauge({
  value,
  left,
  right,
  tone,
  label,
}: {
  value: number;
  left: ReactNode;
  right: ReactNode;
  tone?: "good" | "bad";
  label: string;
}) {
  return (
    <div
      className={`gauge${tone != null ? ` ${tone}` : ""}`}
      role="img"
      aria-label={label}
    >
      <div className="bar">
        <i style={{ width: pct(value) }} />
      </div>
      <div className="cap num">
        <span>{left}</span>
        <span>{right}</span>
      </div>
    </div>
  );
}

interface Pill {
  text: string;
  tone: string;
  title?: string;
}

/** A debt's head: its ring, its name with what it is as a row of
 * marks, and what it holds. The marks replace the sentence a subtitle
 * used to spell out: colour says what the rate is like, the ring says
 * how far along it is. */
function DebtHead({
  debt: d,
  hue,
  assumed,
  cur,
}: {
  debt: Debt;
  hue: string;
  assumed: number;
  cur: string;
}) {
  const loan = d.kind === "installment";
  const tone = rateTone(d.rate, assumed);
  const pills: Pill[] = [];
  if (tone != null && d.rate != null) {
    pills.push({
      text: `${ratio(d.rate)} a year`,
      tone,
      title:
        tone === "free"
          ? "No interest at all."
          : tone === "cheap"
            ? `Less than the ${ratio(assumed)} a portfolio is assumed to make.`
            : tone === "dear"
              ? `More than the ${ratio(assumed)} a portfolio is assumed to make.`
              : "A card's kind of rate: every month carried costs real money.",
    });
  } else {
    pills.push({ text: "no interest seen", tone: "plain" });
  }
  if (loan && d.collateral != null) {
    pills.push({
      text: `secured on ${d.collateral.label}`,
      tone: "plain",
      title: "The lender can take this if the loan is not paid.",
    });
  }
  if (!loan) {
    const c = d.cycle;
    if (d.owed < -EPS) {
      pills.push({
        text: "in credit",
        tone: "calm",
        title:
          "A refund or a payment landed after the balance was cleared; the next statement starts from here.",
      });
    } else if (c?.in_full === true) {
      pills.push({ text: "cleared last cycle", tone: "good" });
    } else if ((c?.carried ?? 0) > EPS) {
      pills.push({
        text: `carrying ${fmtCompact(c?.carried ?? 0, cur)}`,
        tone: "bad",
        title: "Left over after the last payment, and charging interest.",
      });
    }
    if (d.foreign.length > 0) {
      pills.push({
        text: `billed in ${d.foreign.map((f) => f.code).join(", ")}`,
        tone: "plain",
      });
    }
  }
  const ring = loan
    ? {
        value: d.progress ?? 0,
        text: d.progress != null ? ratio(d.progress) : "—",
        label: `${ratio(d.progress ?? 0)} of the loan paid off`,
      }
    : {
        value: d.utilisation ?? 0,
        text: d.utilisation != null ? ratio(d.utilisation) : "—",
        label:
          d.utilisation != null
            ? `${ratio(d.utilisation)} of the limit used`
            : "No limit in the ledger",
      };
  const note = loan
    ? `of ${fmt(d.peak, cur, 0)} borrowed`
    : d.limit != null
      ? `of a ${fmt(d.limit, cur, 0)} limit`
      : d.owed <= EPS
        ? "nothing owed"
        : "on the card";
  return (
    <div className="debt-head">
      <Ring value={ring.value} hue={hue} text={ring.text} label={ring.label} />
      <div className="debt-title">
        <h2>{d.label}</h2>
        <div className="pills">
          {pills.map((p) => (
            <span key={p.text} className={`pill ${p.tone}`} title={p.title}>
              {p.text}
            </span>
          ))}
        </div>
      </div>
      <div className="card-fig">
        <span className="card-total num">{fmt(d.owed, cur)}</span>
        <span className="card-note">{note}</span>
      </div>
    </div>
  );
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

/** What a carried balance is made of: the card's colour fading from
 * the biggest part to the smallest, then a row per part with what it
 * was for, how long it has been there, and what it has cost. */
function MakeupRows({ m, cur }: { m: Makeup; cur: string }) {
  const total = Math.max(m.total, EPS);
  const fade = (i: number) => Math.max(0.3, 1 - i * 0.2);
  return (
    <div className="makeup">
      <div
        className="stack-bar makeup-bar"
        role="img"
        aria-label="What the carried balance is made of"
      >
        {m.rows.map((r, i) => (
          <span
            key={r.account || "other"}
            className="seg"
            style={{ width: pct(r.owed / total), opacity: fade(i) }}
            title={`${r.label} · ${fmt(r.owed, cur)}`}
          />
        ))}
      </div>
      {m.rows.map((r, i) => (
        <div key={r.account || "other"} className="makeup-row num">
          <i className="sw" style={{ opacity: fade(i) }} aria-hidden="true" />
          <div className="l">
            <span className="who">{r.label}</span>
            <span className="hint">
              {r.count === 1 ? "one charge" : `${r.count} charges`} · since{" "}
              {monthYear(r.since)}
            </span>
          </div>
          <div className="r">
            <span className="owed">{fmt(r.owed, cur)}</span>
            <span className={`hint${r.interest > EPS ? " int" : ""}`}>
              {r.interest > EPS
                ? `${fmt(r.interest, cur)} interest`
                : "no interest yet"}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

/** A carried card's months as pairs of bars: what went on it against
 * what came off it. The balance shrinks only where the second bar is
 * the taller. The months the pace is read over are drawn in full;
 * the ones before them are faded. */
function TreadmillChart({ t, cur }: { t: Treadmill; cur: string }) {
  const [ref, W] = useWidth<HTMLDivElement>();
  const H = 124;
  const TOP = 18;
  const BOT = 18;
  const PAD = 8;
  const n = t.months.length;
  const top = Math.max(
    1e-9,
    ...t.months.flatMap((m) => [m.charges + m.interest, m.payments]),
  );
  const slot = (W - 2 * PAD) / Math.max(1, n);
  const gap = Math.max(2, Math.min(6, slot * 0.08));
  const bw = Math.max(3, (slot - 3 * gap) / 2);
  const y = (v: number) => TOP + ((top - v) * (H - TOP - BOT)) / top;
  const recent = new Set(t.months.slice(-t.pace).map((m) => m.month));
  const caps = slot >= 64;
  return (
    <div className="treadmill" ref={ref}>
      <svg
        className="chart-svg"
        width={W}
        height={H}
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label="What went on the card against what came off it, by month"
      >
        <line x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} className="grid zero" />
        {t.months.map((m, i) => {
          const x0 = PAD + i * slot + gap;
          const on = m.charges + m.interest;
          return (
            <g key={m.month} className={`tm${recent.has(m.month) ? "" : " old"}`}>
              <title>
                {`${monthYear(m.month)}\ncharged ${fmt(m.charges, cur)}` +
                  (m.interest > EPS ? ` + ${fmt(m.interest, cur)} interest` : "") +
                  `\npaid ${fmt(m.payments, cur)}`}
              </title>
              <rect
                x={x0}
                y={y(m.charges)}
                width={bw}
                height={Math.max(0, y(0) - y(m.charges))}
                className="tm-charges"
              />
              {m.interest > EPS && (
                <rect
                  x={x0}
                  y={y(on)}
                  width={bw}
                  height={Math.max(0, y(m.charges) - y(on))}
                  className="tm-interest"
                />
              )}
              <rect
                x={x0 + bw + gap}
                y={y(m.payments)}
                width={bw}
                height={Math.max(0, y(0) - y(m.payments))}
                className="tm-pay"
              />
              {caps && on > EPS && (
                <text x={x0 + bw / 2} y={y(on) - 4} textAnchor="middle" className="tm-cap">
                  {fmtCompact(on, cur)}
                </text>
              )}
              {caps && m.payments > EPS && (
                <text
                  x={x0 + bw + gap + bw / 2}
                  y={y(m.payments) - 4}
                  textAnchor="middle"
                  className="tm-cap"
                >
                  {fmtCompact(m.payments, cur)}
                </text>
              )}
              <text x={x0 + bw + gap / 2} y={H - 5} textAnchor="middle" className="axis">
                {monthShort(m.month)}
              </text>
            </g>
          );
        })}
      </svg>
      <Key
        items={[
          { sw: "tm-charges", label: "charged" },
          { sw: "tm-interest", label: "interest" },
          { sw: "tm-pay", label: "paid" },
        ]}
      />
    </div>
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

/** The payment the reader is trying out, and what it would do. */
function ExtraSlider({
  payment,
  month,
  cur,
  extra,
  setExtra,
  base,
  faster,
  what,
}: {
  payment: number;
  month: string;
  cur: string;
  extra: number;
  setExtra: (v: number) => void;
  base: Amortization;
  faster: Amortization | null;
  /** What the usual payment is called: "a month" or "minimum". */
  what: string;
}) {
  const range = sliderRange(payment);
  const saved = faster != null ? base.interest - faster.interest : 0;
  const reads: Read[] =
    faster == null
      ? [
          { v: fmt(payment, cur, 0), l: what },
          { v: monthYear(addMonths(month, base.months)), l: "paid off" },
          { v: fmt(base.interest, cur, 0), l: "interest to come" },
        ]
      : [
          { v: fmt(payment + extra, cur, 0), l: what },
          { v: monthYear(addMonths(month, faster.months)), l: "paid off" },
          { v: duration(base.months - faster.months), l: "sooner", tone: "ok" },
          ...(saved >= 1
            ? [{ v: fmt(saved, cur, 0), l: "less interest", tone: "ok" as const }]
            : []),
        ];
  return (
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
        <span className="slider-val num">
          {extra > 0 ? `+${fmt(extra, cur, 0)}` : "nothing extra"}
        </span>
      </label>
      <Readout items={reads} />
    </div>
  );
}

function LoanCard({
  debt: d,
  hue,
  month,
  assumed,
  cur,
}: {
  debt: Debt;
  hue: string;
  month: string;
  assumed: number;
  cur: string;
}) {
  const [extra, setExtra] = useState(0);
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
  const c = d.collateral;
  const ltv = c?.value != null && c.value > EPS ? d.owed / c.value : null;
  const gap = (c?.value ?? 0) - d.owed;
  const call = d.owed > EPS ? verdict(d.rate, assumed) : null;

  return (
    <div className={`report-card debt-card ${hue}`}>
      <DebtHead debt={d} hue={hue} assumed={assumed} cur={cur} />
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
          lbl="Paid so far"
          v={fmt(paid, cur, 0)}
          hint={`from a peak of ${fmt(d.peak, cur, 0)}`}
        />
        <Fact
          lbl="Interest so far"
          v={fmt(d.interest_paid, cur, 0)}
          tone={d.interest_paid > EPS ? "bad" : undefined}
          hint={
            d.payoff != null
              ? d.payoff.interest > EPS
                ? `${fmt(d.payoff.interest, cur, 0)} still to come`
                : "none still to come"
              : d.payment != null && d.owed > EPS
                ? "no end at this payment"
                : "none to come"
          }
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
      {c != null && d.owed > EPS && (
        <div className="debt-secured">
          {ltv != null && (
            <Gauge
              value={ltv}
              left={`loan ${fmtCompact(d.owed, cur)} · ${ratio(ltv)} of it`}
              right={`${c.label} ${fmtCompact(c.value ?? 0, cur)} · ${fmtCompact(Math.abs(gap), cur)} ${gap >= 0 ? "clear" : "short"}`}
              tone={ltv <= 0.8 ? "good" : ltv >= 1 ? "bad" : undefined}
              label={`The loan against what secures it: ${ratio(ltv)}. Selling ${c.label} would ${gap >= 0 ? "clear the loan with" : "leave the loan"} ${fmt(Math.abs(gap), cur, 0)} ${gap >= 0 ? "to spare" : "short"}.`}
            />
          )}
        </div>
      )}
      {call != null && <Verdict {...call} />}
      {d.payment != null && base != null && d.owed > EPS && (
        <ExtraSlider
          payment={d.payment}
          month={month}
          cur={cur}
          extra={extra}
          setExtra={setExtra}
          base={base}
          faster={faster}
          what="a month"
        />
      )}
      {d.payments.length > 0 && (
        <Payments rows={d.payments.slice(0, 6)} cur={cur} />
      )}
    </div>
  );
}

function RevolvingCard({
  debt: d,
  hue,
  month,
  today,
  from,
  assumed,
  cur,
}: {
  debt: Debt;
  hue: string;
  month: string;
  today: string;
  /** The first month of the interest window. */
  from: string | null;
  assumed: number;
  cur: string;
}) {
  const [extra, setExtra] = useState(0);
  const c = d.cycle;
  const carried = (c?.carried ?? 0) > EPS && d.owed > EPS;
  const base =
    carried && d.payment != null
      ? amortize(d.owed, d.rate ?? 0, d.payment)
      : null;
  const faster =
    carried && d.payment != null && extra > 0
      ? amortize(d.owed, d.rate ?? 0, d.payment + extra)
      : null;
  const fx = foreignText(d.foreign, d.owed, cur);
  const u = d.utilisation;
  const tread = treadmillFacts(d, cur);

  return (
    <div className={`report-card debt-card ${hue}`}>
      <DebtHead debt={d} hue={hue} assumed={assumed} cur={cur} />
      <TrailChart trail={d.trail} today={today} cur={cur} />
      <Key
        items={[
          { sw: "debt", label: "on the card" },
          { sw: "pay", label: "payment" },
        ]}
      />
      {d.limit != null && u != null && (
        <Gauge
          value={u}
          left={`${ratio(u)} of the ${fmtCompact(d.limit, cur)} limit`}
          right={`${fmtCompact(Math.max(0, d.limit - Math.max(0, d.owed)), cur)} left`}
          tone={u <= 0.3 ? "good" : u >= 0.9 ? "bad" : undefined}
          label={`Limit used: ${ratio(u)}`}
        />
      )}
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
                ? `${fmt(d.payment, cur)} ${carried ? "minimum" : "last time"}`
                : "none seen yet"
          }
        />
        <Fact
          lbl="Interest paid"
          v={fmt(d.interest_paid, cur, 0)}
          tone={d.interest_paid > EPS ? "bad" : undefined}
          hint={
            d.interest_paid > EPS && from != null
              ? `since ${monthYear(from)}`
              : d.rate != null
                ? `${ratio(d.rate)} a year`
                : "none charged"
          }
        />
      </div>
      {fx != null && <Prose text={fx} className="small fx" />}
      {d.makeup != null && (
        <div className="debt-section">
          <div className="lbl">What is carried</div>
          <MakeupRows m={d.makeup} cur={cur} />
        </div>
      )}
      {d.treadmill != null && (
        <div className="debt-section">
          <div className="lbl">On the treadmill</div>
          <TreadmillChart t={d.treadmill} cur={cur} />
          {tread != null && (
            <div className="debt-facts tread-facts">
              {tread.map((f) => (
                <Fact key={f.lbl} lbl={f.lbl} v={f.v} hint={f.hint} tone={f.tone} />
              ))}
            </div>
          )}
        </div>
      )}
      {base != null && d.payment != null && (
        <ExtraSlider
          payment={d.payment}
          month={month}
          cur={cur}
          extra={extra}
          setExtra={setExtra}
          base={base}
          faster={faster}
          what="minimum"
        />
      )}
    </div>
  );
}

interface Track {
  order: Order;
  title: string;
  hint: string;
  p: Plan | null;
}

/** Two orders to pay the same debts in, drawn as bars on one clock:
 * each debt runs from now to the month it ends, and the two finish
 * lines are there to compare. */
function Race({
  debts,
  tracks,
  month,
  hue,
  cur,
}: {
  debts: PlanDebt[];
  tracks: Track[];
  month: string;
  hue: Map<string, string>;
  cur: string;
}) {
  const [ref, W] = useWidth<HTMLDivElement>();
  const PAD = 8;
  const LABEL = Math.min(150, Math.round(W * 0.24));
  const HEAD = 24;
  const ROW = 20;
  const GAP = 14;
  const AXIS = 22;
  const n = debts.length;
  const longest = Math.max(0, ...tracks.map((t) => t.p?.months ?? 0));
  const M = Math.max(6, (longest > 0 ? longest : 24) + 1);
  const x0 = LABEL + PAD;
  const x1 = W - PAD;
  const x = (m: number) => x0 + (Math.min(m, M) / M) * (x1 - x0);
  const trackH = HEAD + n * ROW;
  const H = tracks.length * trackH + GAP * (tracks.length - 1) + AXIS;
  const every = M > 60 ? 12 : M > 30 ? 6 : M > 14 ? 3 : 1;
  const axis = Array.from({ length: M }, (_, i) => i + 1)
    .filter((m) => m % every === 0)
    .map((m) => {
      const key = addMonths(month, m);
      return { m, text: key.endsWith("-01") || every >= 12 ? monthYear(key) : monthShort(key) };
    });
  const ends = (p: Plan | null, account: string) =>
    p?.steps.find((s) => s.account === account)?.months ?? null;

  return (
    <div
      className="race"
      ref={ref}
      title="Each bar runs from now to the month the debt ends. When one ends, its payment rolls on to the next."
    >
    <svg
      className="chart-svg race"
      width={W}
      height={H}
      viewBox={`0 0 ${W} ${H}`}
      role="img"
      aria-label="When each debt ends, in each order; when one ends, its payment rolls on to the next"
    >
      {axis.map((a) => (
        <g key={a.m}>
          <line x1={x(a.m)} x2={x(a.m)} y1={0} y2={H - AXIS + 4} className="grid" />
          <text x={x(a.m)} y={H - 6} textAnchor="middle" className="axis">
            {a.text}
          </text>
        </g>
      ))}
      <line x1={x0} x2={x0} y1={0} y2={H - AXIS + 4} className="grid now" />
      <text x={x0} y={H - 6} textAnchor="start" className="axis">
        now
      </text>
      {tracks.map((t, ti) => {
        const top = ti * (trackH + GAP);
        const rows = attack(debts, t.order);
        const summary =
          t.p != null
            ? `${monthYear(addMonths(month, t.p.months))} · ${duration(t.p.months)} · ${fmt(t.p.interest, cur, 0)} interest`
            : "never: the payments do not beat the interest";
        return (
          <g key={t.order}>
            <text x={0} y={top + 13} className="race-head">
              {t.title}
              <tspan className="hint"> · {t.hint}</tspan>
            </text>
            <text x={x1} y={top + 13} textAnchor="end" className="race-sum">
              {summary}
            </text>
            {t.p != null && (
              <line
                x1={x(t.p.months)}
                x2={x(t.p.months)}
                y1={top + HEAD - 4}
                y2={top + trackH}
                className="race-finish"
              />
            )}
            {rows.map((d, i) => {
              const y = top + HEAD + i * ROW;
              const end = ends(t.p, d.account);
              const bx1 = end != null ? x(end) : x1;
              const wide = bx1 - x0 > 84;
              const text = end != null ? monthYear(addMonths(month, end)) : "never";
              const inside = end != null && bx1 > x1 - 70;
              return (
                <g key={d.account} className={`race-row ${hueOf(hue, d.account)}`}>
                  <title>
                    {end != null
                      ? `${d.label}: paid off ${monthYear(addMonths(month, end))}, in ${duration(end)}`
                      : `${d.label}: never, at this payment`}
                  </title>
                  <text x={0} y={y + 14} className="race-name">
                    {cut(d.label, LABEL < 150 ? 14 : 22)}
                  </text>
                  <rect
                    x={x0}
                    y={y + 4}
                    width={Math.max(4, bx1 - x0)}
                    height={12}
                    rx={6}
                    className={`race-bar${end == null ? " never" : ""}`}
                  />
                  {wide && d.rate > EPS && (
                    <text x={x0 + 8} y={y + 14} className="race-in">
                      {ratio(d.rate)}
                    </text>
                  )}
                  <text
                    x={inside ? bx1 - 6 : bx1 + 7}
                    y={y + 14}
                    textAnchor={inside ? "end" : "start"}
                    className={`race-end${inside ? " in" : ""}`}
                  >
                    {text}
                  </text>
                </g>
              );
            })}
          </g>
        );
      })}
    </svg>
    </div>
  );
}

const ORDERS: { order: Order; title: string; hint: string }[] = [
  { order: "avalanche", title: "Highest rate first", hint: "the least interest" },
  { order: "snowball", title: "Smallest first", hint: "the quickest win" },
];

/** Two orders to pay the same debts in, side by side, at whatever
 * extra the reader can find each month. */
function OrderCard({
  debts,
  month,
  hue,
  spare,
  cur,
}: {
  debts: PlanDebt[];
  month: string;
  hue: Map<string, string>;
  spare: Extra;
  cur: string;
}) {
  const usual = debts.reduce((s, d) => s + d.payment, 0);
  const range = sliderRange(Math.max(usual, spare.monthly));
  const [extra, setExtra] = useState(() => sliderStart(spare.monthly, range));
  const a = plan(debts, extra, "avalanche");
  const s = plan(debts, extra, "snowball");
  const saves = a != null && s != null ? s.interest - a.interest : 0;
  // Where the usual month's surplus falls on the slider: the thumb's
  // centre runs from half its width in to half its width short.
  const usualAt = spare.monthly > EPS ? Math.min(1, spare.monthly / range.max) : null;
  const tickAt =
    usualAt != null
      ? `calc(${(usualAt * 100).toFixed(2)}% + ${(8 - 16 * usualAt).toFixed(1)}px)`
      : undefined;
  const reads: Read[] =
    extra > 0
      ? [
          { v: fmt(usual, cur, 0), l: "usual" },
          { v: `+${fmt(extra, cur, 0)}`, l: "extra" },
          { v: fmt(usual + extra, cur, 0), l: "a month" },
        ]
      : [{ v: fmt(usual, cur, 0), l: "the usual payments" }];
  const tracks: Track[] = ORDERS.map((o) => ({
    ...o,
    p: o.order === "avalanche" ? a : s,
  }));

  return (
    <div className="report-card order-card">
      <div className="card-head">
        <h2>Which first?</h2>
        {saves > EPS ? (
          <div className="card-fig">
            <span className="card-total num ok">{fmt(saves, cur, 0)}</span>
            <span className="card-note">saved by the dearest first</span>
          </div>
        ) : (
          a != null && (
            <div className="card-fig">
              <span className="card-total num">
                {monthYear(addMonths(month, a.months))}
              </span>
              <span className="card-note">debt-free either way</span>
            </div>
          )
        )}
      </div>
      <div className={`debt-slider${tickAt != null ? " ticked" : ""}`}>
        <label>
          <span className="lbl">Extra each month</span>
          <span className="range">
            <input
              type="range"
              min={0}
              max={range.max}
              step={range.step}
              value={extra}
              onChange={(e) => setExtra(Number(e.currentTarget.value))}
              aria-valuetext={`${fmt(extra, cur, 0)} more a month`}
            />
            {tickAt != null && (
              <>
                <i
                  className="tick"
                  style={{ left: tickAt }}
                  title={`${fmt(spare.monthly, cur, 0)}: what is usually left over each month`}
                />
                <span className="tick-lbl" style={{ left: tickAt }} aria-hidden="true">
                  usual
                </span>
              </>
            )}
          </span>
          <span className="slider-val num">
            {extra > 0 ? `+${fmt(extra, cur, 0)}` : "nothing extra"}
          </span>
        </label>
        <Readout items={reads} />
      </div>
      <Race debts={debts} tracks={tracks} month={month} hue={hue} cur={cur} />
    </div>
  );
}

/** One beaten debt as a receipt: what it was at its worst, how long it
 * took, and what it cost. */
function Trophy({ b, cur }: { b: Beaten; cur: string }) {
  const paid = b.principal_paid + b.interest_paid;
  const months = Math.max(
    0,
    Math.round(daysBetween(b.first, b.last) / 30.4375),
  );
  return (
    <div className="trophy" title={beatenText(b, cur)}>
      <div className="trophy-top">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
          <path d="M8 1a7 7 0 1 1 0 14A7 7 0 0 1 8 1zm3.1 4.4L7 9.5 4.9 7.4 3.8 8.5 7 11.7l5.2-5.2-1.1-1.1z" />
        </svg>
        <span className="who">{b.label}</span>
      </div>
      <div className="peak num">{fmt(b.peak, cur, 0)}</div>
      <div className="span num">
        {monthYear(b.first.slice(0, 7))} – {monthYear(b.last.slice(0, 7))}
        {months > 0 && <span className="muted"> · {duration(months)}</span>}
      </div>
      {paid > EPS && (
        <div className="split-bar" aria-hidden="true">
          <i className="principal" style={{ width: pct(b.principal_paid / paid) }} />
          <i className="interest" style={{ width: pct(b.interest_paid / paid) }} />
        </div>
      )}
      <div className="trophy-foot num">
        {b.interest_paid > EPS ? (
          <>
            <b>{fmt(b.interest_paid, cur)}</b> interest ·{" "}
            {ratio(b.interest_paid / Math.max(b.peak, EPS))} of it
          </>
        ) : (
          "not a cent in interest"
        )}
      </div>
    </div>
  );
}

/** The receipts: every debt that reached zero, and what it cost. */
function BeatenCard({ beaten, cur }: { beaten: Beaten[]; cur: string }) {
  return (
    <div className="report-card beaten-card">
      <div className="card-head">
        <h2>Debts you've beaten</h2>
        {beaten.length > 1 && (
          <div className="card-fig">
            <span className="card-total num ok">
              {fmt(
                beaten.reduce((s, b) => s + b.peak, 0),
                cur,
                0,
              )}
            </span>
            <span className="card-note">paid off all told</span>
          </div>
        )}
      </div>
      <div className="trophies">
        {beaten.map((b) => (
          <Trophy key={b.account} b={b} cur={cur} />
        ))}
      </div>
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
  // Loans go beside cards. With only one kind in the ledger there is
  // no second group to pair with, so that group is spread across the
  // two columns itself rather than stacked down the middle.
  const only = loans.length === 0 ? cards : cards.length === 0 ? loans : null;
  const spread = only != null ? columns(only, tall) : null;
  const card = (d: Debt) =>
    d.kind === "installment" ? (
      <LoanCard
        key={d.account}
        debt={d}
        hue={hueOf(hue, d.account)}
        month={data.month}
        assumed={data.assumed_return}
        cur={cur}
      />
    ) : (
      <RevolvingCard
        key={d.account}
        debt={d}
        hue={hueOf(hue, d.account)}
        month={data.month}
        today={today}
        from={from}
        assumed={data.assumed_return}
        cur={cur}
      />
    );
  const down = payingDown(data.debts);
  const from = data.interest.window?.[0] ?? null;
  const hue = hues(data.debts);
  // The two planning cards: what to send now, and in what order to
  // clear the lot. Either can be missing, and then the other takes
  // the whole width rather than sitting in half of it.
  const plan =
    data.debts.length > 0 &&
    (data.upcoming.length > 0 || data.extra.now > EPS);
  const race = down.length > 1;

  return (
    <section id="reports" className="debts">
      <Unpriced codes={data.unpriced} cur={cur} where="every figure here" />
      <Masthead data={data} hue={hue} today={today} cur={cur} />
      {data.notices.length > 0 && (
        <div className="notices">
          {data.notices.map((n) => (
            <NoticeRow key={`${n.kind}${n.account}`} n={n} cur={cur} />
          ))}
        </div>
      )}
      {(plan || race) && (
        <div className={`report-grid even${plan && race ? "" : " one"}`}>
          {plan && <PlanCard data={data} hue={hue} today={today} cur={cur} />}
          {race && (
            <OrderCard
              debts={down}
              month={data.month}
              hue={hue}
              spare={data.extra}
              cur={cur}
            />
          )}
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
      ) : spread != null && only != null ? (
        <div className={`report-grid even${spread.length < 2 ? " one" : ""}`}>
          <div className="col-head span">
            <span className="lbl">{loans.length === 0 ? "Cards" : "Loans"}</span>
            <span className="num">
              {fmtCompact(loans.length === 0 ? data.revolving : data.installment, cur)}{" "}
              · {only.length}
            </span>
          </div>
          {spread.map((col) => (
            <div className="report-col" key={col[0]?.account}>
              {col.map(card)}
            </div>
          ))}
        </div>
      ) : (
        <div className="report-grid even">
          <div className="report-col">
            <div className="col-head">
              <span className="lbl">Loans</span>
              <span className="num">
                {fmtCompact(data.installment, cur)} · {loans.length}
              </span>
            </div>
            {loans.map(card)}
          </div>
          <div className="report-col">
            <div className="col-head">
              <span className="lbl">Cards</span>
              <span className="num">
                {fmtCompact(data.revolving, cur)} · {cards.length}
              </span>
            </div>
            {cards.map(card)}
          </div>
        </div>
      )}
      {data.beaten.length > 0 && <BeatenCard beaten={data.beaten} cur={cur} />}
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
