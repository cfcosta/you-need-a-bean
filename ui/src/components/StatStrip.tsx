import type { MonthView } from "../api";
import { fmt, windowLabel } from "../format";

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
  const pctTyp =
    view.typical != null && view.typical > 0
      ? Math.round((view.spent / view.typical) * 100)
      : null;
  const pctMon =
    view.days_in_month > 0
      ? Math.round((view.day / view.days_in_month) * 100)
      : 0;

  return (
    <div id="strip">
      <div className="tile">
        <div className="lbl">Income{sofar}</div>
        <div className="val num">{fmt(view.income, cur)}</div>
        <div className="sub">all Income accounts</div>
      </div>
      <div className="tile">
        <div className="lbl">Spent{sofar}</div>
        <div className="val num">{fmt(view.spent, cur)}</div>
        <div className="sub">
          {pctTyp != null
            ? `${pctTyp}% of a typical month`
            : "no earlier months to compare"}
          {view.is_current ? ` · ${pctMon}% of the month gone` : ""}
        </div>
      </div>
      <div className="tile">
        <div className="lbl">Typical month</div>
        <div className="val num">
          {view.typical != null ? fmt(view.typical, cur) : "—"}
        </div>
        <div className="sub">
          median month with payments, {windowLabel(window)}
        </div>
      </div>
      <div className="tile">
        <div className="lbl">Net{sofar}</div>
        <div className="val num">
          <span className={net >= 0 ? "pos" : ""}>{fmt(net, cur)}</span>
        </div>
        <div className="sub">income − spending</div>
      </div>
    </div>
  );
}
