import type { MouseEvent } from "react";

import type { AccountRow, MonthView, Summary } from "../api";
import { fmt } from "../format";
import type { Page } from "../router";
import { THEMES } from "../theme";
import type { ThemePref } from "../theme";

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
  active,
  href,
  onOpen,
}: {
  title: string;
  rows: AccountRow[];
  cur: string;
  active: string | null;
  href: (account: string) => string;
  onOpen: (account: string) => void;
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
          <a
            key={a.account}
            className={`acct-row${a.account === active ? " active" : ""}`}
            title={a.account}
            href={href(a.account)}
            aria-current={a.account === active ? "page" : undefined}
            onClick={(e) => {
              if (!plain(e)) return;
              e.preventDefault();
              onOpen(a.account);
            }}
          >
            <span className="acct-name">
              <span className="p">{a.label}</span>
            </span>
            <span className={`acct-amt num${shown < 0 ? " neg" : ""}`}>
              {single != null ? fmt(shown, single) : `≈ ${fmt(shown, cur)}`}
            </span>
          </a>
        );
      })}
    </div>
  );
}

const THEME_LABEL: Record<ThemePref, string> = {
  auto: "Auto",
  light: "Day",
  dark: "Night",
};
const THEME_TITLE: Record<ThemePref, string> = {
  auto: "follow the system's colour scheme",
  light: "the light scheme",
  dark: "the dark scheme",
};

function ThemeIcon({ which }: { which: ThemePref }) {
  if (which === "light") {
    return (
      <svg width="11" height="11" viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="3" fill="none" stroke="currentColor" strokeWidth="1.6" />
        <path
          d="M8 1v2M8 13v2M1 8h2M13 8h2M3.05 3.05l1.4 1.4M11.55 11.55l1.4 1.4M3.05 12.95l1.4-1.4M11.55 4.45l1.4-1.4"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
        />
      </svg>
    );
  }
  if (which === "dark") {
    return (
      <svg width="11" height="11" viewBox="0 0 16 16" aria-hidden="true">
        <path
          d="M13.6 10.2A5.8 5.8 0 0 1 5.8 2.4a6.2 6.2 0 1 0 7.8 7.8z"
          fill="currentColor"
        />
      </svg>
    );
  }
  return (
    <svg width="11" height="11" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" strokeWidth="1.6" />
      <path d="M8 2.6a5.4 5.4 0 0 0 0 10.8z" fill="currentColor" />
    </svg>
  );
}

/** Night, day, or whatever the system says. The choice is remembered
 * per browser; it is a fact about the screen, not about the ledger. */
function ThemeSwitch({
  theme,
  onTheme,
}: {
  theme: ThemePref;
  onTheme: (pref: ThemePref) => void;
}) {
  return (
    <div className="side-foot">
      <div className="theme-seg" role="group" aria-label="Colour scheme">
        {THEMES.map((t) => (
          <button
            key={t}
            type="button"
            className={t === theme ? "on" : ""}
            aria-pressed={t === theme}
            title={THEME_TITLE[t]}
            onClick={() => onTheme(t)}
          >
            <ThemeIcon which={t} />
            {THEME_LABEL[t]}
          </button>
        ))}
      </div>
    </div>
  );
}

export function Sidebar({
  summary,
  view,
  cur,
  open,
  page,
  acct,
  href,
  acctHref,
  onNavigate,
  onAccount,
  theme,
  onTheme,
}: {
  summary: Summary;
  view: MonthView | null;
  cur: string;
  open: boolean;
  page: Page;
  /** The account whose register is on screen, so its row can say so. */
  acct: string | null;
  href: (page: Page) => string;
  acctHref: (account: string) => string;
  onNavigate: (page: Page) => void;
  onAccount: (account: string) => void;
  /** The colour scheme in force, and the switch for it. */
  theme: ThemePref;
  onTheme: (pref: ThemePref) => void;
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
          <div className="wordmark-text">you need a bean</div>
        </div>
        <div
          className="ledger-chip mono"
          title={`read-only — beancount is the source of truth\n${summary.directives.toLocaleString(
            "en-US",
          )} directives across ${summary.files} files, read in ${parseLabel(
            summary.parse_ms,
          )}`}
        >
          <svg
            className="lock"
            width="10"
            height="10"
            viewBox="0 0 16 16"
            fill="currentColor"
            aria-label="read-only"
          >
            <path d="M8 1a3.2 3.2 0 0 0-3.2 3.2V6H4a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V7a1 1 0 0 0-1-1h-.8V4.2A3.2 3.2 0 0 0 8 1zm1.8 5H6.2V4.2a1.8 1.8 0 1 1 3.6 0V6z" />
          </svg>
          <b>{summary.title ?? summary.root ?? "ledger"}</b>
          <span className="lc-n num">
            {summary.directives.toLocaleString("en-US")}
          </span>
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
            active={page === "account" ? acct : null}
            href={acctHref}
            onOpen={onAccount}
          />
          <AccountList
            title="Tracking"
            rows={view.accounts.tracking}
            cur={cur}
            active={page === "account" ? acct : null}
            href={acctHref}
            onOpen={onAccount}
          />
        </>
      )}

      <ThemeSwitch theme={theme} onTheme={onTheme} />
    </nav>
  );
}
