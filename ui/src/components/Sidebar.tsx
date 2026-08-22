import type { MouseEvent } from "react";

import type { AccountRow, MonthView, Summary } from "../api";
import { fmt } from "../format";
import type { Page } from "../router";

/** A click the app should handle itself, rather than letting the browser
 * open a tab or a window. */
const plain = (e: MouseEvent) =>
  e.button === 0 && !e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey;

function parseLabel(ms: number): string {
  return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
}

function AccountList({
  title,
  rows,
  cur,
}: {
  title: string;
  rows: AccountRow[];
  cur: string;
}) {
  // The server already hides zero-balance accounts.
  if (rows.length === 0) return null;
  const total = rows.reduce((s, a) => s + (a.converted ?? 0), 0);
  return (
    <div className="acct-section">
      <div className="acct-title">
        <span>{title}</span>
        <span className="tot num" title={`converted to ${cur}`}>
          ≈ {fmt(total, cur, 0)}
        </span>
      </div>
      {rows.map((a) => {
        const codes = Object.keys(a.balances);
        const single = codes.length === 1 ? codes[0] : null;
        const shown =
          single != null ? (a.balances[single] ?? 0) : (a.converted ?? 0);
        return (
          <button key={a.account} className="acct-row" title={a.account}>
            <span className="acct-name">
              <span className="p">{a.label}</span>
            </span>
            <span className={`acct-amt num${shown < 0 ? " neg" : ""}`}>
              {single != null ? fmt(shown, single) : `≈ ${fmt(shown, cur)}`}
            </span>
          </button>
        );
      })}
    </div>
  );
}

export function Sidebar({
  summary,
  view,
  cur,
  open,
  page,
  href,
  onNavigate,
}: {
  summary: Summary;
  view: MonthView | null;
  cur: string;
  open: boolean;
  page: Page;
  href: (page: Page) => string;
  onNavigate: (page: Page) => void;
}) {
  return (
    <nav
      id="sidebar"
      className={open ? "open" : ""}
      aria-label="Ledger navigation"
    >
      <div className="side-head">
        <div className="wordmark">
          <svg width="26" height="26" viewBox="0 0 32 32" aria-hidden="true">
            <ellipse
              cx="16"
              cy="16"
              rx="9.5"
              ry="12.5"
              transform="rotate(-22 16 16)"
              fill="var(--bean)"
            />
            <path
              d="M11.5 6.5 C 19.5 12.5, 12.5 19.5, 20.5 25.5"
              stroke="var(--brand)"
              strokeWidth="2.8"
              fill="none"
              strokeLinecap="round"
            />
          </svg>
          <div className="wordmark-text">
            you need a bean<small>budget from beancount</small>
          </div>
        </div>
        <div
          className="ledger-chip mono"
          title={'option "title" from the main ledger file'}
        >
          <b>{summary.title ?? summary.root ?? "ledger"}</b>
          <br />
          {summary.root ?? "?"} ·{" "}
          {summary.files === 1 ? "1 file" : `${summary.files} files`}
          <br />
          {summary.directives.toLocaleString("en-US")} directives · parsed{" "}
          {parseLabel(summary.parse_ms)}
        </div>
      </div>

      <div className="side-nav">
        <a
          className={`nav-item${page === "budget" ? " active" : ""}`}
          href={href("budget")}
          aria-current={page === "budget" ? "page" : undefined}
          onClick={(e) => {
            if (!plain(e)) return;
            e.preventDefault();
            onNavigate("budget");
          }}
        >
          <svg
            width="13"
            height="13"
            viewBox="0 0 16 16"
            fill="currentColor"
            aria-hidden="true"
          >
            <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h11A1.5 1.5 0 0 1 15 3.5v9a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9zM2.5 5v7.5h11V5h-11z" />
          </svg>
          Budget
        </a>
        <a
          className={`nav-item${page === "reports" ? " active" : ""}`}
          href={href("reports")}
          aria-current={page === "reports" ? "page" : undefined}
          onClick={(e) => {
            if (!plain(e)) return;
            e.preventDefault();
            onNavigate("reports");
          }}
        >
          <svg
            width="13"
            height="13"
            viewBox="0 0 16 16"
            fill="currentColor"
            aria-hidden="true"
          >
            <path d="M1 14h14v1H1v-1zm2-4h2v3H3v-3zm4-5h2v8H7V5zm4 2h2v6h-2V7z" />
          </svg>
          Reports
        </a>
      </div>

      {view && (
        <>
          <AccountList
            title="Budget accounts"
            rows={view.accounts.budget}
            cur={cur}
          />
          <AccountList
            title="Tracking"
            rows={view.accounts.tracking}
            cur={cur}
          />
        </>
      )}

      <div className="side-foot mono">
        read-only · beancount is the source of truth
      </div>
    </nav>
  );
}
