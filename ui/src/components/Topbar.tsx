import type { MonthView, Summary } from "../api";
import { monthName, windowLabel } from "../format";
import type { Page } from "../router";
import { monthProgress } from "../strip";

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
  page: Page;
  onMonth: (m: string) => void;
  onBasis: (b: number) => void;
  onCur: (c: string) => void;
  onBurger: () => void;
}) {
  const months = summary.months;
  const i = months.indexOf(month);
  const prev = i > 0 ? months[i - 1] : null;
  const next = i >= 0 && i < months.length - 1 ? months[i + 1] : null;

  const title =
    page === "reports" ? "Reports" : page === "liabilities" ? "Liabilities" : null;

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
      {title != null ? (
        <div className="month-label page-title">
          <h1>{title}</h1>
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
          <div
            className="month-progress"
            title={
              view == null
                ? undefined
                : view.is_current
                  ? `day ${view.day} of ${view.days_in_month}`
                  : "complete month"
            }
          >
            <span
              style={{
                width:
                  view == null
                    ? "0%"
                    : `${((monthProgress(view) ?? 1) * 100).toFixed(1)}%`,
              }}
            />
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
        {/* The same window, read two ways: the months a target is
            taken from, and the months an account's chart covers. */}
        <span className="seg-label">
          {page === "account" || page === "liabilities" ? "History" : "Target"}
        </span>
        <div className="seg" role="group" aria-label="Average window">
          {BASES.map((b) => (
            <button
              key={b}
              className={b === basis ? "on" : ""}
              title={
                page === "account"
                  ? `The last ${b} months of this account`
                  : page === "liabilities"
                    ? `The last ${b} months of each debt`
                    : b === basis
                    ? `The median month with spending in the previous ${b} (${windowLabel(window)})`
                    : `The median month with spending in the previous ${b}`
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
