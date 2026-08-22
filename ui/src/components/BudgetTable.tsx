import type { CategoryRow, MonthView, Status } from "../api";
import { fmt, fmtCode, monthName, pctLabel, windowLabel } from "../format";

/** The bullet bar spans 0..128% of the average; the notch sits at 100%. */
const CAP = 1.28;
const MARK = 100 / CAP;

/** How the row should look even when the API has no status (no average). */
export function displayStatus(row: {
  status: Status | null;
  spent: number;
}): Status {
  return row.status ?? (row.spent > 0 ? "over" : "good");
}

function VsBar({
  row,
  cur,
  basis,
  pacePos,
  day,
  daysIn,
}: {
  row: CategoryRow;
  cur: string;
  basis: number;
  pacePos: number | null;
  day: number;
  daysIn: number;
}) {
  const st = displayStatus(row);
  const fillPct =
    row.avg != null && row.avg > 0 && row.ratio != null
      ? (Math.min(row.ratio, CAP) / CAP) * 100
      : 0;
  const over = row.avg != null ? row.avg - row.spent < 0 : false;
  const tip =
    row.avg == null
      ? `${fmt(row.spent, cur)} spent — no earlier spending to average`
      : over
        ? `${fmt(row.spent, cur)} spent — ${fmt(row.spent - row.avg, cur)} over the ${basis}-mo average of ${fmt(row.avg, cur)}`
        : `${fmt(row.spent, cur)} spent of ${fmt(row.avg, cur)} typical`;
  return (
    <span className="vs-cell">
      <span className={`vsbar ${st}`} title={tip}>
        <span className="fill" style={{ width: `${fillPct}%` }} />
        <span className="avg-mark" style={{ left: `${MARK}%` }} />
        {pacePos != null && (
          <span
            className="pace"
            style={{ left: `${pacePos}%` }}
            title={`Day ${day} of ${daysIn} — even-pace mark`}
          />
        )}
      </span>
      <span className={`vs-pct num ${st}`}>{pctLabel(row.ratio)}</span>
    </span>
  );
}

function Row({
  c,
  label,
  solo,
  selected,
  cur,
  basis,
  pacePos,
  day,
  daysIn,
  onSelect,
}: {
  c: CategoryRow;
  label: string;
  solo: boolean;
  selected: boolean;
  cur: string;
  basis: number;
  pacePos: number | null;
  day: number;
  daysIn: number;
  onSelect: (account: string) => void;
}) {
  const mixed = Object.keys(c.split).filter((k) => k !== cur);
  const fxTip = Object.entries(c.split)
    .map(([k, v]) => fmtCode(v, k))
    .join("  +  ");
  return (
    <button
      className={`cat-row${solo ? " solo" : ""}${selected ? " sel" : ""}`}
      aria-selected={selected}
      title={c.account}
      onClick={() => onSelect(c.account)}
    >
      <span className="cat-name">
        <span className="p">{label}</span>
      </span>
      <span className="cell num dim">
        {c.avg != null ? fmt(c.avg, cur) : "—"}
      </span>
      <span className="cell num">
        {fmt(c.spent, cur)}
        {mixed.length > 0 && (
          <span className="fx" title={`Native amounts: ${fxTip}`}>
            +{mixed.join("+")}
          </span>
        )}
      </span>
      <VsBar
        row={c}
        cur={cur}
        basis={basis}
        pacePos={pacePos}
        day={day}
        daysIn={daysIn}
      />
    </button>
  );
}

export function BudgetTable({
  view,
  cur,
  basis,
  window,
  selected,
  closedGroups,
  onSelect,
  onToggleGroup,
}: {
  view: MonthView;
  cur: string;
  basis: number;
  window: [string, string] | null;
  selected: string | null;
  closedGroups: ReadonlySet<string>;
  onSelect: (account: string) => void;
  onToggleGroup: (name: string) => void;
}) {
  const pacePos = view.is_current
    ? (view.day / view.days_in_month) * MARK
    : null;

  return (
    <div id="table-card">
      <div className="thead">
        <div>Category</div>
        <div>
          <span
            className="hint"
            title={`Average of months with spending in the previous ${basis} (${windowLabel(window)})`}
          >
            Typical / mo
          </span>
        </div>
        <div>Spent</div>
        <div>
          <span
            className="hint"
            title={`Fill = spent this month · notch = the ${basis}-month average · red = past it`}
          >
            vs typical
          </span>
        </div>
      </div>
      <div>
        {view.groups.length === 0 && (
          <div className="empty">
            No expense categories in {monthName(view.month)}.
          </div>
        )}
        {view.groups.map((g) => {
          // A group with a single category collapses into one row.
          const solo = g.categories.length === 1 ? g.categories[0] : null;
          if (solo != null) {
            return (
              <Row
                key={g.name}
                c={solo}
                label={
                  solo.label === g.name ? g.name : `${g.name} · ${solo.label}`
                }
                solo
                selected={selected === solo.account}
                cur={cur}
                basis={basis}
                pacePos={pacePos}
                day={view.day}
                daysIn={view.days_in_month}
                onSelect={onSelect}
              />
            );
          }
          const closed = closedGroups.has(g.name);
          return (
            <div key={g.name}>
              <button
                className={`grp-head num${closed ? " closed" : ""}`}
                aria-expanded={!closed}
                onClick={() => onToggleGroup(g.name)}
              >
                <span className="g-name">
                  <span className="chev">
                    <svg
                      width="9"
                      height="9"
                      viewBox="0 0 16 16"
                      fill="currentColor"
                      aria-hidden="true"
                    >
                      <path d="M2 5l6 6 6-6H2z" />
                    </svg>
                  </span>
                  {g.name}
                </span>
                <span className="g-amt dim">
                  {g.avg != null ? fmt(g.avg, cur) : "—"}
                </span>
                <span className="g-amt">{fmt(g.spent, cur)}</span>
                <span className="g-amt">
                  {g.avg != null && g.avg > 0
                    ? `${Math.round((g.spent / g.avg) * 100)}%`
                    : "—"}
                </span>
              </button>
              {!closed &&
                g.categories.map((c) => (
                  <Row
                    key={c.account}
                    c={c}
                    label={c.label}
                    solo={false}
                    selected={selected === c.account}
                    cur={cur}
                    basis={basis}
                    pacePos={pacePos}
                    day={view.day}
                    daysIn={view.days_in_month}
                    onSelect={onSelect}
                  />
                ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
