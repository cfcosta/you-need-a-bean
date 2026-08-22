import type { MonthView, Summary } from "../api";
import { monthName, windowLabel } from "../format";

const BASES = [3, 6, 12] as const;

export function Topbar({
  summary,
  view,
  month,
  basis,
  cur,
  window,
  page,
  onMonth,
  onBasis,
  onCur,
  onBurger,
}: {
  summary: Summary;
  view: MonthView | null;
  month: string;
  basis: number;
  cur: string;
  window: [string, string] | null;
  page: "budget" | "reports";
  onMonth: (m: string) => void;
  onBasis: (b: number) => void;
  onCur: (c: string) => void;
  onBurger: () => void;
}) {
  const months = summary.months;
  const i = months.indexOf(month);
  const prev = i > 0 ? months[i - 1] : null;
  const next = i >= 0 && i < months.length - 1 ? months[i + 1] : null;

  return (
    <div id="topbar">
      <button id="burger" aria-label="Open navigation" onClick={onBurger}>
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="currentColor"
          aria-hidden="true"
        >
          <path d="M1 3h14v1.5H1V3zm0 4.25h14v1.5H1v-1.5zM1 11.5h14V13H1v-1.5z" />
        </svg>
      </button>
      {page === "reports" ? (
        <div className="month-label page-title">
          <h1>Reports</h1>
          <div className="sub num">as of {summary.today}</div>
        </div>
      ) : (
      <div className="month-pager">
        <button
          className="pager-btn"
          aria-label="Previous month"
          disabled={prev == null}
          onClick={() => prev != null && onMonth(prev)}
        >
          <svg
            width="11"
            height="11"
            viewBox="0 0 16 16"
            fill="currentColor"
            aria-hidden="true"
          >
            <path d="M10.5 2 5 8l5.5 6 1-1L7 8l4.5-5-1-1z" />
          </svg>
        </button>
        <div className="month-label">
          <h1>{monthName(month)}</h1>
          <div className="sub num">
            {view == null
              ? " "
              : view.is_current
                ? `day ${view.day} of ${view.days_in_month}`
                : "complete month"}
          </div>
        </div>
        <button
          className="pager-btn"
          aria-label="Next month"
          disabled={next == null}
          onClick={() => next != null && onMonth(next)}
        >
          <svg
            width="11"
            height="11"
            viewBox="0 0 16 16"
            fill="currentColor"
            aria-hidden="true"
          >
            <path d="M5.5 2 11 8l-5.5 6-1-1L9 8 4.5 3l1-1z" />
          </svg>
        </button>
      </div>
      )}
      <div className="topbar-right">
        <span className="seg-label">Target</span>
        <div className="seg" role="group" aria-label="Average window">
          {BASES.map((b) => (
            <button
              key={b}
              className={b === basis ? "on" : ""}
              title={
                b === basis
                  ? `Average of months with spending in the previous ${b} (${windowLabel(window)}); one-off spikes count as 3× the median`
                  : `Average of months with spending in the previous ${b}; one-off spikes count as 3× the median`
              }
              onClick={() => onBasis(b)}
            >
              {b} mo
            </button>
          ))}
        </div>
        {summary.operating_currencies.length > 1 && (
          <>
            <span className="seg-label">Currency</span>
            <div className="seg" role="group" aria-label="Operating currency">
              {summary.operating_currencies.map((c) => (
                <button
                  key={c}
                  className={c === cur ? "on" : ""}
                  onClick={() => onCur(c)}
                >
                  {c}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
