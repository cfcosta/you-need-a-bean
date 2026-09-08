import { useEffect, useRef, useState } from "react";

import type {
  AccountView,
  CategoryView,
  MonthView,
  LiabilitiesView,
  ReportsView,
  Summary,
} from "./api";
import {
  getAccount,
  getCategory,
  getMonth,
  getLiabilities,
  getReports,
  getSummary,
} from "./api";
import { InvestmentsPage } from "./components/Investments";
import { Home } from "./components/Home";
import { Search } from "./components/Search";
import { Account } from "./components/Account";
import { BudgetTable } from "./components/BudgetTable";
import { Inspector } from "./components/Inspector";
import { Liabilities } from "./components/Liabilities";
import { Reports } from "./components/Reports";
import { Sidebar } from "./components/Sidebar";
import {
  AccountSkeleton,
  BootSkeleton,
  Fatal,
  LiabilitiesSkeleton,
  ReportsSkeleton,
} from "./components/Skeleton";
import { StatStrip } from "./components/StatStrip";
import { Topbar } from "./components/Topbar";
import { monthWindow } from "./months";
import type { Page } from "./router";
import { DEFAULT_BASIS, defaultCurrency, parseRoute, resolveRoute, routeUrl } from "./router";
import type { ThemePref } from "./theme";
import { applyTheme, browserStorage, loadPref, savePref } from "./theme";

// How often we ask the server whether it has reread the ledger. Cheap
// enough to go unnoticed, quick enough that a save feels immediate.
const RELOAD_POLL_MS = 1500;

export function App() {
  const [summary, setSummary] = useState<Summary | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);
  const [disconnected, setDisconnected] = useState(false);
  const pollFailures = useRef(0);
  const [page, setPage] = useState<Page>("home");
  const [search, setSearch] = useState<string | null>(null);
  const [month, setMonth] = useState<string | null>(null);
  const [basis, setBasis] = useState(DEFAULT_BASIS);
  const cur = summary == null ? null : defaultCurrency(summary);
  const [cat, setCat] = useState<string | null>(null);
  const [acct, setAcct] = useState<string | null>(null);
  // The auto-picked category is an implementation detail; only one the
  // reader chose is worth putting in the URL.
  const [catChosen, setCatChosen] = useState(false);
  const [view, setView] = useState<MonthView | null>(null);
  const [reports, setReports] = useState<ReportsView | null>(null);
  const [debts, setDebts] = useState<LiabilitiesView | null>(null);
  const [catView, setCatView] = useState<CategoryView | null>(null);
  const [acctView, setAcctView] = useState<AccountView | null>(null);
  const [openTxns, setOpenTxns] = useState<ReadonlySet<number>>(new Set());
  const [closedGroups, setClosedGroups] = useState<ReadonlySet<string>>(
    new Set(),
  );
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [theme, setTheme] = useState<ThemePref>(() =>
    loadPref(browserStorage()),
  );
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Where the URL last pointed, so the next write knows whether it is a
  // navigation (new history entry) or a change of view (rewrite).
  const wasAt = useRef<{
    page: Page;
    month: string;
    acct: string | null;
  } | null>(null);
  // What the last poll saw of the server's reading of the ledger, so a
  // reload is noticed exactly once.
  const seen = useRef<{ revision: number; error: string | null; today: string } | null>(null);

  useEffect(() => {
    const key = (e: KeyboardEvent) => { if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); setSearch(""); } };
    addEventListener("keydown", key); return () => removeEventListener("keydown", key);
  }, []);

  const showToast = (msg: string) => {
    setToast(msg);
    if (toastTimer.current != null) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 2600);
  };

  useEffect(() => {
    let alive = true;
    getSummary()
      .then((s) => {
        if (!alive) return;
        // The URL decides where we open; the ledger decides what of it
        // makes sense.
        const r = resolveRoute(parseRoute(location), s);
        seen.current = { revision: s.revision, error: s.reload_error, today: s.today };
        setCatChosen(r.cat != null);
        setSummary(s);
        setPage(r.page);
        setMonth(r.month);
        setBasis(r.basis);
        setCat(r.cat);
        setAcct(r.acct);
        if (s.title != null) document.title = `${s.title} — you need a bean`;
      })
      .catch((e: Error) => alive && setFatal(e.message));
    return () => {
      alive = false;
    };
  }, []);

  // The scheme is a choice about the browser, not the ledger, so it lives
  // on the document and in local storage rather than in the URL.
  useEffect(() => {
    applyTheme(theme, document.documentElement);
    savePref(theme, browserStorage());
  }, [theme]);

  // The server rereads the ledger when its files change. Follow it, so
  // saving a file in your editor shows up here without a refresh.
  const loaded = summary != null;
  // Every view below is derived from one reading of the ledger, so they
  // all refetch when the server moves to the next one.
  const revision = `${summary?.revision ?? 0}:${summary?.today ?? ""}`;
  useEffect(() => {
    if (!loaded) return;
    let alive = true;
    const timer = setInterval(() => {
      getSummary()
        .then((s) => {
          if (!alive) return;
          pollFailures.current = 0;
          setDisconnected(false);
          const was = seen.current;
          if (
            !alive ||
            (was != null &&
              was.revision === s.revision &&
              was.error === s.reload_error && was.today === s.today)
          ) {
            return;
          }
          if (was != null && s.revision > was.revision) {
            showToast("ledger reloaded");
          }
          seen.current = { revision: s.revision, error: s.reload_error, today: s.today };
          setSummary(s);
        })
        .catch(() => { if (alive && ++pollFailures.current >= 3) setDisconnected(true); });
    }, RELOAD_POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [loaded]);

  // Keep the address bar honest. Moving between pages or months is
  // navigation and earns a history entry; changing basis or
  // category rewrites where you already are.
  useEffect(() => {
    if (summary == null || month == null || cur == null) return;
    const url = routeUrl(
      { page, month, basis, cur, cat: catChosen ? cat : null, acct },
      summary,
    );
    if (url !== location.pathname + location.search) {
      const moved =
        wasAt.current != null &&
        (wasAt.current.page !== page ||
          wasAt.current.month !== month ||
          wasAt.current.acct !== acct);
      if (moved) history.pushState(null, "", url);
      else history.replaceState(null, "", url);
    }
    wasAt.current = { page, month, acct };
  }, [summary, page, month, basis, cur, cat, catChosen, acct]);

  useEffect(() => {
    if (summary == null) return;
    const onPop = () => {
      const r = resolveRoute(parseRoute(location), summary);
      // The browser already moved us, so this is where we now are: the
      // writer above must not answer it with another history entry.
      wasAt.current = { page: r.page, month: r.month, acct: r.acct };
      setCatChosen(r.cat != null);
      setPage(r.page);
      setMonth(r.month);
      setBasis(r.basis);
      setCat(r.cat);
      setAcct(r.acct);
      setOpenTxns(new Set());
    };
    addEventListener("popstate", onPop);
    return () => removeEventListener("popstate", onPop);
  }, [summary]);

  useEffect(() => {
    if (month == null || cur == null) return;
    let alive = true;
    setView(null);
    getMonth(month, basis, cur)
      .then((v) => alive && setView(v))
      .catch((e: Error) => alive && showToast(e.message));
    return () => {
      alive = false;
    };
  }, [month, basis, cur, revision]);

  useEffect(() => {
    if (page !== "reports" || cur == null) return;
    let alive = true;
    setReports(null);
    getReports(basis, cur)
      .then((r) => alive && setReports(r))
      .catch((e: Error) => alive && showToast(e.message));
    return () => {
      alive = false;
    };
  }, [page, basis, cur, revision]);

  useEffect(() => {
    if (page !== "liabilities" || cur == null) return;
    let alive = true;
    setDebts(null);
    getLiabilities(basis, cur)
      .then((r) => alive && setDebts(r))
      .catch((e: Error) => alive && showToast(e.message));
    return () => {
      alive = false;
    };
  }, [page, basis, cur, revision]);

  useEffect(() => {
    if (cat != null || view == null) return;
    const first = view.groups[0]?.categories[0];
    if (first != null) setCat(first.account);
  }, [cat, view]);

  useEffect(() => {
    if (cat == null || month == null || cur == null) return;
    let alive = true;
    setCatView(null);
    getCategory(cat, month, basis, cur)
      .then((v) => alive && setCatView(v))
      .catch((e: Error) => {
        if (!alive) return;
        setCatView(null);
        showToast(e.message);
      });
    return () => {
      alive = false;
    };
  }, [cat, month, basis, cur, revision]);

  useEffect(() => {
    if (acct == null || month == null || cur == null) return;
    let alive = true;
    setAcctView(null);
    getAccount(acct, month, basis, cur)
      .then((v) => alive && setAcctView(v))
      .catch((e: Error) => {
        if (!alive) return;
        setAcctView(null);
        showToast(e.message);
      });
    return () => {
      alive = false;
    };
  }, [acct, month, basis, cur, revision]);

  if (fatal != null) return <Fatal why={fatal} />;
  if (summary == null || month == null || cur == null) return <BootSkeleton />;

  const window = monthWindow(summary.months, month, basis);

  const selectMonth = (m: string) => {
    setMonth(m);
    setOpenTxns(new Set());
  };
  const selectCat = (account: string) => {
    setCatChosen(true);
    setCat(account);
    setOpenTxns(new Set());
    if (matchMedia("(max-width: 1200px)").matches) setInspectorOpen(true);
  };
  const toggleGroup = (name: string) => {
    setClosedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };
  const toggleTxn = (i: number) => {
    setOpenTxns((prev) => {
      const next = new Set(prev);
      if (next.has(i)) next.delete(i);
      else next.add(i);
      return next;
    });
  };

  const navigate = (p: Page) => {
    if (p === "home" || p === "reports" || p === "liabilities" || p === "investments") setMonth(summary.default_month);
    setPage(p);
    setSidebarOpen(false);
  };
  const openAccount = (account: string) => {
    if (page === "home" || page === "investments") setMonth(summary.default_month);
    setPage("account");
    setAcct(account);
    setOpenTxns(new Set());
    setSidebarOpen(false);
  };
  // Real links in the nav, so the browser's own gestures work.
  const hrefFor = (p: Page) =>
    routeUrl(
      { page: p, month, basis, cur, cat: catChosen ? cat : null, acct },
      summary,
    );
  const acctHref = (account: string) =>
    routeUrl(
      { page: "account", month, basis, cur, cat: null, acct: account },
      summary,
    );

  return (
    <>
      <div id="app" className={page === "budget" ? "" : `no-insp${page === "home" ? " home-app" : page === "investments" ? " investments-app" : ""}`}>
        <Sidebar
          summary={summary}
          view={view}
          cur={cur}
          open={sidebarOpen}
          page={page}
          acct={acct}
          href={hrefFor}
          acctHref={acctHref}
          onNavigate={navigate}
          onAccount={openAccount}
          theme={theme}
          onTheme={setTheme}
        />
        <main id="main">
          <Topbar
            summary={summary}
            view={view}
            month={month}
            basis={basis}
            window={window}
            page={page}
            onMonth={selectMonth}
            onBasis={setBasis}
            onBurger={() => setSidebarOpen(true)}
            onSearch={() => setSearch("")}
          />
          {disconnected && <div className="accounting-alert" role="status"><b>Connection lost</b> · These are the last numbers received. Reconnecting…</div>}
          {summary.audit.issues.length > 0 && (
            <details className="accounting-alert">
              <summary><b>{summary.audit.issues.length} accounting issues</b><span> These totals need review</span></summary>
              <p>Some balances may be incomplete. Resolve these issues in the ledger before using the projections.</p>
              {summary.audit.issues.map((issue, i) => (
                <div className="accounting-issue" key={i}>
                  <span>{issue.message}</span>
                  {issue.source && <code>{issue.source.path}:{issue.source.line}</code>}
                </div>
              ))}
            </details>
          )}
          {summary.reload_error != null && (
            <div id="stale" role="status">
              <svg
                width="13"
                height="13"
                viewBox="0 0 16 16"
                fill="currentColor"
                aria-hidden="true"
              >
                <path d="M8 1.4 15.2 14H.8L8 1.4zM7.25 6v4h1.5V6h-1.5zm0 5v1.5h1.5V11h-1.5z" />
              </svg>
              <span className="stale-what">
                the ledger stopped parsing — these are the last numbers that
                worked
              </span>
              <span className="stale-why mono">{summary.reload_error}</span>
            </div>
          )}
          {page === "home" && <Home cur={cur} revision={revision} unavailable={summary.reload_error != null || disconnected} onAccount={openAccount} onSearch={setSearch} />}
          {page === "investments" && <InvestmentsPage cur={cur} today={summary.today} revision={revision} unavailable={disconnected || summary.reload_error != null || summary.audit.issues.length > 0} onAccount={openAccount} onSearch={setSearch} />}
          {page === "budget" && view != null && (
            <>
              <StatStrip view={view} cur={cur} window={window} />
              <BudgetTable
                view={view}
                cur={cur}
                basis={basis}
                window={window}
                selected={cat}
                closedGroups={closedGroups}
                onSelect={selectCat}
                onToggleGroup={toggleGroup}
              />
            </>
          )}
          {page === "reports" &&
            (reports != null ? (
              <Reports data={{...reports, planning_ready: reports.planning_ready && !disconnected && summary.reload_error == null && summary.audit.issues.length === 0}} cur={cur} />
            ) : (
              <ReportsSkeleton />
            ))}
          {page === "liabilities" &&
            (debts != null ? (
              <Liabilities
                data={{...debts, planning_ready: debts.planning_ready && !disconnected && summary.reload_error == null && summary.audit.issues.length === 0}}
                cur={cur}
                today={summary.today}
                basis={basis}
              />
            ) : (
              <LiabilitiesSkeleton />
            ))}
          {page === "account" &&
            (acctView != null && acctView.account === acct ? (
              <Account
                view={acctView}
                cur={cur}
                month={month}
                openTxns={openTxns}
                onToggleTxn={toggleTxn}
              />
            ) : (
              <AccountSkeleton />
            ))}
        </main>
        {page === "budget" && (
          <Inspector
            view={cat != null ? catView : null}
            cur={cur}
            basis={basis}
            month={month}
            open={inspectorOpen}
            openTxns={openTxns}
            onToggleTxn={toggleTxn}
            onClose={() => setInspectorOpen(false)}
          />
        )}
      </div>
      {/* The fills the charts share. Blue is money that is yours, purple is
          money the market holds for you, red is money owed; each fades
          toward its baseline so the band reads as a quantity, not a block. */}
      <svg className="defs" aria-hidden="true" focusable="false">
        <defs>
          <linearGradient id="g-cash" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" style={{ stopColor: "var(--accent)" }} stopOpacity="0.55" />
            <stop offset="1" style={{ stopColor: "var(--accent)" }} stopOpacity="0.04" />
          </linearGradient>
          <linearGradient id="g-market" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" style={{ stopColor: "var(--market)" }} stopOpacity="0.6" />
            <stop offset="1" style={{ stopColor: "var(--market)" }} stopOpacity="0.06" />
          </linearGradient>
          <linearGradient id="g-debt" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" style={{ stopColor: "var(--over)" }} stopOpacity="0.08" />
            <stop offset="1" style={{ stopColor: "var(--over)" }} stopOpacity="0.5" />
          </linearGradient>
        </defs>
      </svg>
      {/* Both drawers dim the page behind them, and a tap on the dim
          shuts whichever is open. Below 1200 the inspector is a sheet
          over the page rather than a column beside it, so without this
          the only way out of it is one small ✕. Above 1200 the scrim is
          display:none whatever this says, and the inspector is a column
          that is not open or shut. */}
      <div
        id="scrim"
        className={sidebarOpen || inspectorOpen ? "show" : ""}
        onClick={() => {
          setSidebarOpen(false);
          setInspectorOpen(false);
        }}
      />
      {search != null && <Search key={search} initial={search} cur={cur} onClose={() => setSearch(null)} />}
      <div id="toast" role="status" aria-live="polite" className={toast != null ? "show" : ""}>
        {toast}
      </div>
    </>
  );
}
