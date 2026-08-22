import type { CategoryView, Txn } from "../api";
import { fmt, fmtCode, monthName, monthShort, windowLabel } from "../format";
import { displayStatus } from "./BudgetTable";

const PLOT_H = 94; // px height of the chart's plot area

function Chart({
  view,
  cur,
  month,
}: {
  view: CategoryView;
  cur: string;
  month: string;
}) {
  const values = view.history.map((h) => h.spent);
  const max = Math.max(...values, view.avg ?? 0, 1);
  const avgBottom = view.avg != null ? 16 + (view.avg / max) * PLOT_H : null;
  return (
    <div className="chart" role="img" aria-label="Spending for the last 6 months">
      <div className="cols">
        {view.history.map((h) => {
          const now = h.month === month;
          const bh = Math.max(
            (h.spent / max) * PLOT_H,
            h.spent > 0 ? 3 : 1.5,
          );
          return (
            <div
              key={h.month}
              className={`col${now ? " now" : ""}`}
              title={`${monthName(h.month)}: ${fmt(h.spent, cur)}`}
            >
              {now && (
                <span className="cap num" style={{ bottom: `${bh + 3}px` }}>
                  {fmt(h.spent, cur, 0)}
                </span>
              )}
              <div className="bar" style={{ height: `${bh}px` }} />
            </div>
          );
        })}
      </div>
      <div className="baseline" />
      {avgBottom != null && view.avg != null && (
        <>
          <div className="avg-line" style={{ bottom: `${avgBottom}px` }} />
          <div className="avg-tag num" style={{ bottom: `${avgBottom}px` }}>
            typical {fmt(view.avg, cur, 0)}
          </div>
        </>
      )}
      <div className="xlabels">
        {view.history.map((h) => (
          <span key={h.month}>{monthShort(h.month)}</span>
        ))}
      </div>
    </div>
  );
}

function CurrencyCard({
  view,
  cur,
}: {
  view: CategoryView;
  cur: string;
}) {
  const codes = Object.keys(view.split);
  const only = codes.length === 1 ? codes[0] : null;
  if (codes.length === 0 || (only != null && only === cur)) return null;
  return (
    <div className="card">
      <h3>This month by currency</h3>
      <div className="cur-split num">
        {codes.map((k) => {
          const native = view.split[k] ?? 0;
          let conv = "operating currency";
          if (k !== cur) {
            const inK = view.txns.filter((t) => t.currency === k);
            const convertible = inK.filter((t) => t.converted != null);
            if (inK.length === 0 || convertible.length < inK.length) {
              conv = `no price to ${cur}`;
            } else {
              const sum = convertible.reduce(
                (s, t) => s + (t.converted ?? 0),
                0,
              );
              conv =
                native !== 0
                  ? `= ${fmt(sum, cur)} @ ${(sum / native).toFixed(2)}`
                  : `= ${fmt(sum, cur)}`;
            }
          }
          return (
            <div key={k} className="cur-row">
              <span className="native mono">{fmtCode(native, k)}</span>
              <span className="conv">{conv}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function TxnChips({ txn }: { txn: Txn }) {
  return (
    <>
      {txn.flag === "!" && (
        <span className="badge pend" title="flag: ! (pending)">
          !
        </span>
      )}
      {txn.tags.map((t) => (
        <span key={t} className="badge tag">
          #{t}
        </span>
      ))}
      {txn.links.map((l) => (
        <span key={l} className="badge tag link-chip" title="link">
          ^{l}
        </span>
      ))}
    </>
  );
}

function TxnDetail({ txn, cur }: { txn: Txn; cur: string }) {
  const meta = Object.entries(txn.meta);
  const hasMeta = meta.length > 0 || txn.tags.length > 0 || txn.links.length > 0;
  const rate =
    txn.currency != null &&
    txn.currency !== cur &&
    txn.converted != null &&
    txn.amount != null &&
    txn.amount !== 0
      ? txn.converted / txn.amount
      : null;
  return (
    <div className="txn-detail">
      <div>
        <div className="detail-label">Postings</div>
        <div className="postings mono num">
          <table>
            <tbody>
              {txn.postings.map((p, i) => (
                <tr key={i}>
                  <td>{p.account}</td>
                  <td>
                    {p.amount != null && p.currency != null
                      ? fmtCode(p.amount, p.currency)
                      : ""}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      {hasMeta && (
        <div>
          <div className="detail-label">Metadata</div>
          <dl className="kv mono">
            {meta.map(([k, v]) => (
              <div key={k} style={{ display: "contents" }}>
                <dt>{k}:</dt>
                <dd>"{v}"</dd>
              </div>
            ))}
            {txn.tags.map((t) => (
              <div key={t} style={{ display: "contents" }}>
                <dt>tag</dt>
                <dd>#{t}</dd>
              </div>
            ))}
            {txn.links.map((l) => (
              <div key={l} style={{ display: "contents" }}>
                <dt>link</dt>
                <dd>^{l}</dd>
              </div>
            ))}
          </dl>
        </div>
      )}
      {txn.currency != null && txn.currency !== cur && txn.converted != null && (
        <div>
          <div className="detail-label">Conversion</div>
          <div className="kv mono">
            <dt>{txn.amount != null ? fmtCode(txn.amount, txn.currency) : "—"}</dt>
            <dd>
              = {fmt(txn.converted, cur)}
              {rate != null ? ` @ ${rate.toFixed(2)}` : ""}
            </dd>
          </div>
        </div>
      )}
    </div>
  );
}

export function Inspector({
  view,
  cur,
  basis,
  month,
  open,
  openTxns,
  onToggleTxn,
  onClose,
}: {
  view: CategoryView | null;
  cur: string;
  basis: number;
  month: string;
  open: boolean;
  openTxns: ReadonlySet<number>;
  onToggleTxn: (i: number) => void;
  onClose: () => void;
}) {
  return (
    <aside
      id="inspector"
      className={open ? "open" : ""}
      aria-label="Detail panel"
    >
      <button className="insp-close" aria-label="Close panel" onClick={onClose}>
        ✕
      </button>
      <div className="insp-pad">
        {view == null ? (
          <div className="empty">Select a category to inspect it.</div>
        ) : (
          <InspectorBody
            view={view}
            cur={cur}
            basis={basis}
            month={month}
            openTxns={openTxns}
            onToggleTxn={onToggleTxn}
          />
        )}
      </div>
    </aside>
  );
}

function InspectorBody({
  view,
  cur,
  basis,
  month,
  openTxns,
  onToggleTxn,
}: {
  view: CategoryView;
  cur: string;
  basis: number;
  month: string;
  openTxns: ReadonlySet<number>;
  onToggleTxn: (i: number) => void;
}) {
  const group = view.account.split(":")[1] ?? "";
  const st = displayStatus(view);
  const left = view.avg != null ? view.avg - view.spent : null;
  const meterPct =
    view.avg != null && view.avg > 0
      ? Math.min((view.spent / view.avg) * 100, 100)
      : view.spent > 0
        ? 100
        : 0;
  const pct =
    view.avg != null && view.avg > 0
      ? `${Math.round((view.spent / view.avg) * 100)}%`
      : "—";

  return (
    <>
      <div className="insp-head">
        <div className="crumb">
          {group} · {monthName(month)}
        </div>
        <h2>{view.label}</h2>
        <div className="raw mono">{view.account}</div>
      </div>

      <div className="card">
        <h3>Target — typical month</h3>
        <div className="target-val num">
          {view.avg != null ? fmt(view.avg, cur) : "—"}
          <span className="target-sub"> / month</span>
        </div>
        <div className="target-sub">
          {view.window != null
            ? `median month with payments in ${windowLabel(view.window)} (${basis} mo)`
            : "no earlier months with payments yet"}
        </div>
        <div className={`target-meter meter ${st}`}>
          <span className="fill" style={{ width: `${meterPct}%` }} />
        </div>
        <div className="target-legend num">
          <span>
            spent <b>{fmt(view.spent, cur)}</b>
          </span>
          <span>
            {left != null && left < 0 ? (
              <b style={{ color: "var(--over-text)" }}>
                {fmt(-left, cur)} over · {pct}
              </b>
            ) : (
              <>
                <b>{pct}</b> of typical
              </>
            )}
          </span>
        </div>
      </div>

      <div className="card">
        <h3>Last 6 months</h3>
        <Chart view={view} cur={cur} month={month} />
      </div>

      <CurrencyCard view={view} cur={cur} />

      <div>
        <div className="detail-label" style={{ margin: "4px 0 2px" }}>
          Transactions — {monthName(month)} ({view.txns.length})
        </div>
        <div className="txn-list">
          {view.txns.length === 0 && (
            <div className="empty">No transactions in {monthName(month)}.</div>
          )}
          {view.txns.map((t, ti) => {
            const isOpen = openTxns.has(ti);
            return (
              <div key={ti}>
                <button
                  className={`txn${isOpen ? " open" : ""}`}
                  aria-expanded={isOpen}
                  onClick={() => onToggleTxn(ti)}
                >
                  <span className="d num mono">{t.date.slice(8)}</span>
                  <span className="who">
                    <span className="payee">
                      {t.payee ?? t.narration ?? "(no description)"}{" "}
                      <TxnChips txn={t} />
                    </span>
                    {t.payee != null && t.narration != null && (
                      <span className="narr">{t.narration}</span>
                    )}
                  </span>
                  <span className="amt num">
                    {t.amount != null && t.currency != null
                      ? fmt(t.amount, t.currency)
                      : "—"}
                    {t.currency != null &&
                      t.currency !== cur &&
                      t.converted != null && (
                        <span className="conv">= {fmt(t.converted, cur)}</span>
                      )}
                  </span>
                </button>
                {isOpen && <TxnDetail txn={t} cur={cur} />}
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
}
