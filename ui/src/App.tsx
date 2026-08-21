import { useEffect, useRef, useState } from "react";

import type { CategoryView, MonthView, Summary } from "./api";
import { getCategory, getMonth, getSummary } from "./api";
import { BudgetTable } from "./components/BudgetTable";
import { Inspector } from "./components/Inspector";
import { Sidebar } from "./components/Sidebar";
import { StatStrip } from "./components/StatStrip";
import { Topbar } from "./components/Topbar";
import { monthWindow } from "./months";

export function App() {
  const [summary, setSummary] = useState<Summary | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);
  const [month, setMonth] = useState<string | null>(null);
  const [basis, setBasis] = useState(6);
  const [cur, setCur] = useState<string | null>(null);
  const [cat, setCat] = useState<string | null>(null);
  const [view, setView] = useState<MonthView | null>(null);
  const [catView, setCatView] = useState<CategoryView | null>(null);
  const [openTxns, setOpenTxns] = useState<ReadonlySet<number>>(new Set());
  const [closedGroups, setClosedGroups] = useState<ReadonlySet<string>>(
    new Set(),
  );
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

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
        setSummary(s);
        setMonth(s.default_month);
        setCur(s.operating_currencies[0] ?? "USD");
        if (s.title != null) document.title = `${s.title} — you need a bean`;
      })
      .catch((e: Error) => alive && setFatal(e.message));
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    if (month == null || cur == null) return;
    let alive = true;
    getMonth(month, basis, cur)
      .then((v) => alive && setView(v))
      .catch((e: Error) => alive && showToast(e.message));
    return () => {
      alive = false;
    };
  }, [month, basis, cur]);

  useEffect(() => {
    if (cat != null || view == null) return;
    const first = view.groups[0]?.categories[0];
    if (first != null) setCat(first.account);
  }, [cat, view]);

  useEffect(() => {
    if (cat == null || month == null || cur == null) return;
    let alive = true;
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
  }, [cat, month, basis, cur]);

  if (fatal != null) {
    return <div className="empty">cannot reach the ledger server: {fatal}</div>;
  }
  if (summary == null || month == null || cur == null) {
    return <div className="empty">loading ledger…</div>;
  }

  const window = monthWindow(summary.months, month, basis);

  const selectMonth = (m: string) => {
    setMonth(m);
    setOpenTxns(new Set());
  };
  const selectCat = (account: string) => {
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

  return (
    <>
      <div id="app">
        <Sidebar
          summary={summary}
          view={view}
          cur={cur}
          open={sidebarOpen}
          onToast={showToast}
        />
        <main id="main">
          <Topbar
            summary={summary}
            view={view}
            month={month}
            basis={basis}
            cur={cur}
            window={window}
            onMonth={selectMonth}
            onBasis={setBasis}
            onCur={setCur}
            onBurger={() => setSidebarOpen(true)}
          />
          {view != null && (
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
        </main>
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
      </div>
      <div
        id="scrim"
        className={sidebarOpen ? "show" : ""}
        onClick={() => setSidebarOpen(false)}
      />
      <div id="toast" role="status" aria-live="polite" className={toast != null ? "show" : ""}>
        {toast}
      </div>
    </>
  );
}
