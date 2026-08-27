import type { AccountPoint, AccountView, RegisterTxn } from "../api";
import { fmt, fmtCode, fmtCompact, monthName, monthShort } from "../format";
import { counterpart, prevMonth } from "../register";
import { TxnChips, TxnDetail } from "./Inspector";

const W = 720;
const H = 168;
const TOP = 14;
const BOT = 18;
const PAD = 8;

/** The month as a bank draws it: what arrived above the line, what left
 * below it, and the net of the two as a line across both. All three
 * share one scale, so the picture never has to be read twice. */
function FlowChart({
  points,
  cur,
  month,
}: {
  points: AccountPoint[];
  cur: string;
  month: string;
}) {
  const n = points.length;
  if (n === 0) return <div className="empty">no months to chart</div>;
  const max = Math.max(...points.flatMap((p) => [p.inflow, p.outflow]), 1);
  const half = (H - TOP - BOT) / 2;
  const mid = TOP + half;
  const reach = half - 5;
  const h = (v: number) => (v > 0 ? Math.max((v / max) * reach, 2) : 0);
  const slot = (W - 2 * PAD) / n;
  const bw = Math.min(24, slot * 0.44);
  const cx = (i: number) => PAD + slot * (i + 0.5);
  const net = (p: AccountPoint) => p.inflow - p.outflow;
  const line = points
    .map(
      (p, i) =>
        `${i === 0 ? "M" : "L"}${cx(i).toFixed(1)},${(
          mid -
          (net(p) / max) * reach
        ).toFixed(1)}`,
    )
    .join("");

  return (
    <>
      <svg
        className="chart-svg"
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={`Money in and out by month, converted to ${cur}`}
      >
        <line x1={PAD} x2={W - PAD} y1={mid} y2={mid} className="grid zero" />
        {points.map((p, i) => {
          const now = p.month === month;
          const cls = now ? "" : " past";
          return (
            <g key={p.month}>
              <rect
                className={`rg-bar in${cls}`}
                x={cx(i) - bw / 2}
                y={mid - h(p.inflow)}
                width={bw}
                height={h(p.inflow)}
                rx="2"
              />
              <rect
                className={`rg-bar out${cls}`}
                x={cx(i) - bw / 2}
                y={mid}
                width={bw}
                height={h(p.outflow)}
                rx="2"
              />
            </g>
          );
        })}
        <path d={line} className="rg-line" />
        {points.map((p, i) => (
          <circle
            key={p.month}
            cx={cx(i)}
            cy={mid - (net(p) / max) * reach}
            r={p.month === month ? 3 : 2}
            className="rg-dot"
          />
        ))}
        {points.map((p, i) => (
          <text
            key={p.month}
            x={cx(i)}
            y={H - 4}
            textAnchor="middle"
            className={`axis${p.month === month ? " on" : ""}`}
          >
            {monthShort(p.month)}
          </text>
        ))}
        {points.map((p, i) => (
          <rect
            key={p.month}
            x={cx(i) - slot / 2}
            y={0}
            width={slot}
            height={H}
            fill="transparent"
          >
            <title>
              {`${monthName(p.month)}\nin ${fmt(p.inflow, cur)} · out ${fmt(
                p.outflow,
                cur,
              )}\nleft ${p.balance != null ? fmt(p.balance, cur) : "—"}`}
            </title>
          </rect>
        ))}
      </svg>
      <div className="key">
        <span className="key-item">
          <i className="sw in" />
          in
        </span>
        <span className="key-item">
          <i className="sw out" />
          out
        </span>
        <span className="key-item" title="what each month added or took away">
          <i className="sw net" />
          net
        </span>
      </div>
    </>
  );
}

function Row({
  txn,
  account,
  cur,
  open,
  onToggle,
}: {
  txn: RegisterTxn;
  account: string;
  cur: string;
  open: boolean;
  onToggle: () => void;
}) {
  const other = counterpart(txn.postings, account);
  const inflow = txn.delta != null && txn.delta > 0 ? txn.delta : null;
  const outflow = txn.delta != null && txn.delta < 0 ? -txn.delta : null;
  return (
    <>
      <button
        className={`reg-row${open ? " open" : ""}`}
        aria-expanded={open}
        onClick={onToggle}
      >
        <span className="d num mono">{txn.date.slice(5)}</span>
        <span className="who">
          <span className="payee">
            {txn.payee ?? txn.narration ?? "(no description)"}{" "}
            <TxnChips txn={txn} />
          </span>
          {txn.payee != null && txn.narration != null && (
            <span className="narr">{txn.narration}</span>
          )}
        </span>
        <span
          className={`cat ${other.kind}`}
          title={other.accounts.join("\n") || undefined}
        >
          {other.label}
        </span>
        <span className="out num">
          {outflow != null ? fmt(outflow, cur) : ""}
        </span>
        <span className="in num">{inflow != null ? fmt(inflow, cur) : ""}</span>
        <span className="bal num">
          {txn.balance != null ? (
            fmt(txn.balance, cur)
          ) : (
            <span className="hole" title="a leg of this has no price">
              —
            </span>
          )}
        </span>
      </button>
      {open && (
        <div className="reg-detail">
          <TxnDetail txn={txn} account={account} cur={cur} />
        </div>
      )}
    </>
  );
}

export function Account({
  view,
  cur,
  month,
  openTxns,
  onToggleTxn,
}: {
  view: AccountView;
  cur: string;
  month: string;
  openTxns: ReadonlySet<number>;
  onToggleTxn: (i: number) => void;
}) {
  const codes = Object.keys(view.balances);
  const native =
    codes.length > 0
      ? codes.map((c) => fmtCode(view.balances[c] ?? 0, c)).join(" · ")
      : null;
  const ins = view.txns.filter((t) => (t.delta ?? 0) > 0).length;
  const outs = view.txns.filter((t) => (t.delta ?? 0) < 0).length;

  return (
    <>
      <div className="acct-head">
        <div className="crumb">
          {view.kind === "tracking" ? "Tracking account" : "Budget account"} ·{" "}
          {monthName(month)}
        </div>
        <h2>{view.label}</h2>
        <div className="raw mono">{view.account}</div>
      </div>

      <div id="strip">
        <div className="tile">
          <div className="lbl">Opening</div>
          <div className="val num">
            {view.opening != null ? fmt(view.opening, cur) : "—"}
          </div>
          <div className="sub">end of {monthName(prevMonth(month))}</div>
        </div>
        <div className="tile">
          <div className="lbl">In</div>
          <div className="val num">
            <span className="pos">{fmt(view.inflow, cur)}</span>
          </div>
          <div className="sub">
            {ins} {ins === 1 ? "arrival" : "arrivals"}
          </div>
        </div>
        <div className="tile">
          <div className="lbl">Out</div>
          <div className="val num">{fmt(view.outflow, cur)}</div>
          <div className="sub">
            {outs} {outs === 1 ? "payment" : "payments"}
          </div>
        </div>
        <div className="tile">
          <div className="lbl">Balance</div>
          <div className="val num">
            {view.balance != null ? fmt(view.balance, cur) : "—"}
          </div>
          <div className="sub" title={native ?? undefined}>
            {native != null && (codes.length > 1 || codes[0] !== cur)
              ? native
              : `end of ${monthShort(month)}`}
          </div>
        </div>
      </div>

      <div id="account">
        {view.unpriced.length > 0 && (
          <div className="notice">
            This month moved <b>{view.unpriced.join(", ")}</b>, which nothing
            prices in {cur}. Every figure here is missing them — the lines
            they touch show a dash rather than a number that isn't one.
          </div>
        )}

        <div className="report-card">
          <div className="card-head">
            <div>
              <h2>Money in and out</h2>
              <div className="card-span">
                {view.history.length} months to {monthShort(month)}
              </div>
            </div>
            <div className="card-fig">
              <span className="card-total num">
                {fmtCompact(view.inflow - view.outflow, cur)}
              </span>
              <span className="card-note">this month, net</span>
            </div>
          </div>
          <FlowChart points={view.history} cur={cur} month={month} />
        </div>

        <div className="report-card">
          <div className="card-head">
            <div>
              <h2>Register</h2>
              <div className="card-span">
                {monthName(month)} · {view.txns.length}{" "}
                {view.txns.length === 1 ? "transaction" : "transactions"}
              </div>
            </div>
          </div>
          <div className="reg">
            <div className="reg-head">
              <span>Date</span>
              <span>Payee</span>
              <span className="cat-h">Category</span>
              <span className="r">Outflow</span>
              <span className="r">Inflow</span>
              <span className="r">Balance</span>
            </div>
            <div className="reg-row opening">
              <span className="d num mono" />
              <span className="who">
                <span className="payee">Opening balance</span>
              </span>
              <span className="cat" />
              <span />
              <span />
              <span className="bal num">
                {view.opening != null ? fmt(view.opening, cur) : "—"}
              </span>
            </div>
            {view.txns.map((t, i) => (
              <Row
                key={i}
                txn={t}
                account={view.account}
                cur={cur}
                open={openTxns.has(i)}
                onToggle={() => onToggleTxn(i)}
              />
            ))}
            {view.txns.length === 0 && (
              <div className="empty">
                Nothing moved through this account in {monthName(month)}.
              </div>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
