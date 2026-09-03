import type { MonthView } from "../api";
import { fmt, pctLabel, windowLabel } from "../format";
import { stripBars } from "../strip";

const pos = (v: number) => `${(v * 100).toFixed(2)}%`;

/** One tile's length on the strip's shared scale. `mark` is the typical
 * month, `pace` where an even spend would have reached by today. */
export function Bar({
  cls,
  fill,
  mark,
  pace,
}: {
  cls: string;
  fill: number;
  mark?: number | null;
  pace?: number | null;
}) {
  return (
    <span className={`tile-bar ${cls}`} aria-hidden="true">
      <span className="fill" style={{ width: pos(fill) }} />
      {pace != null && <span className="pace" style={{ left: pos(pace) }} />}
      {mark != null && <span className="typ" style={{ left: pos(mark) }} />}
    </span>
  );
}

export function StatStrip({
  view,
  cur,
  window,
}: {
  view: MonthView;
  cur: string;
  window: [string, string] | null;
}) {
  const sofar = view.is_current ? " so far" : "";
  const net = view.income - view.spent;
  const bars = stripBars(view);
  const ratio =
    view.typical != null && view.typical > 0
      ? view.spent / view.typical
      : null;
  const spentTip = [
    view.typical != null
      ? `${fmt(view.spent, cur)} of a typical ${fmt(view.typical, cur)} — ${pctLabel(ratio)}`
      : `${fmt(view.spent, cur)} — no earlier months to compare`,
    view.is_current
      ? `day ${view.day} of ${view.days_in_month}`
      : "complete month",
  ].join("\n");

  return (
    <div id="strip">
      <div className="tile">
        <div className="lbl">Income{sofar}</div>
        <div className="val num">{fmt(view.income, cur)}</div>
        <Bar cls="in" fill={bars.income} />
      </div>
      <div className="tile" title={spentTip}>
        <div className="lbl">Spent{sofar}</div>
        <div className="val num">{fmt(view.spent, cur)}</div>
        <Bar
          cls={ratio != null && ratio > 1 ? "out over" : "out"}
          fill={bars.spent}
          mark={bars.typical}
          pace={bars.paceMark}
        />
      </div>
      <div
        className="tile"
        title={`averaged over ${windowLabel(window)}`}
      >
        <div className="lbl">Typical month</div>
        <div className="val num">
          {view.typical != null ? fmt(view.typical, cur) : "—"}
        </div>
        <Bar cls="typical" fill={bars.typical ?? 0} />
      </div>
      <div className="tile">
        <div className="lbl">Net{sofar}</div>
        <div className="val num">
          <span className={net >= 0 ? "pos" : ""}>{fmt(net, cur)}</span>
        </div>
        <Bar cls={bars.negative ? "net neg" : "net"} fill={bars.net} />
      </div>
    </div>
  );
}
