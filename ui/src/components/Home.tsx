import { useEffect, useState } from "react";
import type { Attention, HomeView } from "../api";
import { getHome } from "../api";
import { fmt, fmtCompact, monthYear } from "../format";
import { accountUrl, dayLabel, scenario } from "../home";
import { ReportsSkeleton } from "./Skeleton";

const money = (n:number|null, cur:string) => n == null ? "—" : fmt(n,cur,0);
const pct = (n:number,d:number) => Math.max(0,Math.min(100,d>0?n/d*100:0));
const shortAccount = (s:string) => s.split(":").slice(-2).join(" · ");

function Trajectory({ data, cur }: {data:HomeView;cur:string}) {
  const [horizon,setHorizon] = useState(30);
  const [extra,setExtra] = useState(0);
  const f=data.forecast[String(horizon)]!;
  const points=scenario(f.points,data.today,horizon,extra);
  const low=points.length ? Math.min(...points.map(p=>p.balance)) : null;
  const end=points.at(-1)?.balance ?? null;
  const max=Math.max(...points.map(p=>p.balance),1)*1.08;
  const min=points.length ? Math.min(...points.map(p=>p.balance))-Math.max(100,Math.abs(Math.min(...points.map(p=>p.balance)))*.05) : 0;
  const x=(d:string) => 28+(Date.parse(d)-Date.parse(data.today))/86_400_000/horizon*704;
  const y=(v:number) => 195-(v-min)/(max-min)*155;
  const path=points.map((p,i)=>`${i?'L':'M'}${x(p.date)},${y(p.balance)}`).join(" ");
  const events=data.events.filter(e=>(Date.parse(e.date)-Date.parse(data.today))/86_400_000<=horizon);
  return <article className="h-card h-trajectory">
    <header className="h-card-head"><div><span className="h-eyebrow">LOOKING AHEAD</span><h2>Your next chapter</h2></div>
      <div className="h-segment" role="group" aria-label="Forecast horizon">{[30,60,90].map(n=><button key={n} aria-pressed={horizon===n} onClick={()=>setHorizon(n)}>{n} days</button>)}</div>
    </header>
    <div className="h-projection-stats"><div><span>After known commitments</span><strong>{money(end,cur)}</strong></div><div><span>Lowest projected balance</span><strong className={low!=null&&low<0?'h-red':''}>{money(low,cur)}</strong></div><span className="h-pill">{events.length} money movements</span></div>
    {points.length ? <svg className="h-forecast" viewBox="0 0 760 226" role="img" aria-label={`${horizon}-day cash projection. Lowest balance ${money(low,cur)}. Ending balance ${money(end,cur)}.`}>
      <defs><linearGradient id="home-cash-fill" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="var(--accent)" stopOpacity=".24"/><stop offset="1" stopColor="var(--accent)" stopOpacity="0"/></linearGradient></defs>
      {[.25,.5,.75].map(r=><g key={r}><line x1="28" x2="732" y1={y(min+(max-min)*r)} y2={y(min+(max-min)*r)} stroke="var(--line)" strokeDasharray="3 5"/><text x="732" y={y(min+(max-min)*r)-7} textAnchor="end">{fmtCompact(min+(max-min)*r,cur)}</text></g>)}
      <path d={`${path} L732,202 L28,202 Z`} fill="url(#home-cash-fill)"/>
      <path d={path} fill="none" stroke="var(--accent)" strokeWidth="2.5" strokeLinejoin="round"/>
      {points.map((p,i)=><circle key={i} cx={x(p.date)} cy={y(p.balance)} r="4" fill="var(--surface)" stroke="var(--accent)" strokeWidth="2"><title>{dayLabel(p.date)} · {fmt(p.balance,cur)}</title></circle>)}
      <text x="28" y="223">TODAY</text><text x="732" y="223" textAnchor="end">{dayLabel(points.at(-1)!.date).toUpperCase()}</text>
    </svg> : <div className="h-empty h-chart-empty"><span className="h-orbit">◇</span><strong>A clearer picture comes first</strong><p>Resolve accounting and pricing issues to see a cash projection.</p></div>}
    <div className="h-scenario"><label htmlFor="extra-spending">Make room for life <span>Additional spending / month</span></label><output htmlFor="extra-spending">{fmt(extra,cur,0)}</output>
      <input id="extra-spending" type="range" min="0" max={Math.max(1000,Math.ceil(data.monthly_spend/500)*500)} step="50" value={extra} disabled={!points.length} onChange={e=>setExtra(Number(e.target.value))}/>
    </div>
    <p className="h-caption">Recorded future money and estimated recurring payments. Unrecorded income and other spending are excluded. Adjust the scenario above; your ledger stays unchanged.</p>
  </article>;
}

function AttentionPanel({items,onSearch}:{items:Attention[];onSearch:(q:string)=>void}) {
  const [tab,setTab]=useState("priority");
  const [expanded,setExpanded]=useState(false);
  const coverage=items.filter(i=>i.kind==="coverage");
  const priority=items.filter(i=>i.kind!=="coverage");
  const shown=tab==="priority"?priority:coverage;
  return <article className="h-card h-attention"><header className="h-card-head"><div><span className="h-eyebrow">A LITTLE ATTENTION</span><h2>Keep things in order</h2></div><span className="h-count">{items.length}</span></header>
    <div className="h-tabs"><button aria-pressed={tab==="priority"} onClick={()=>{setTab("priority");setExpanded(false)}}>Review <span>{priority.length}</span></button><button aria-pressed={tab==="coverage"} onClick={()=>{setTab("coverage");setExpanded(false)}}>Source coverage <span>{coverage.length}</span></button></div>
    <div className="h-review-list">{shown.slice(0,expanded?undefined:5).map((i,k)=><details key={k} className="h-review"><summary><span className={`h-issue-dot ${i.kind}`}/><span><b>{i.label}</b><small>{i.detail}</small></span><span className="h-chevron">↗</span></summary><div className="h-review-detail">
      {i.kind==="coverage"&&<p>Transaction activity does not prove an account is up to date. Check its statement, then record source coverage or a balance assertion in the ledger.</p>}
      {i.source&&<code>{i.source.path}:{i.source.line}</code>}
      {i.account&&<button className="h-text-button" onClick={()=>onSearch(i.account!)}>Explore account entries ↗</button>}
    </div></details>)}
    {!shown.length&&<div className="h-empty"><span className="h-orbit">✓</span><strong>{tab==="priority"?"No review items found":"No coverage gaps found"}</strong><p>These checks cover the recorded ledger. They do not independently verify your institutions.</p></div>}</div>
    {shown.length>5&&<button className="h-more" onClick={()=>setExpanded(!expanded)}>{expanded?"Show less":`See all ${shown.length} items`} <span>↓</span></button>}
  </article>;
}

function HomeContent({data,cur,onAccount,onSearch}:{data:HomeView;cur:string;onAccount:(a:string)=>void;onSearch:(q:string)=>void}) {
  const [allAccounts,setAllAccounts]=useState(false);
  const [eventTab,setEventTab]=useState("next");
  const [allEvents,setAllEvents]=useState(false);
  const [accountFilter,setAccountFilter]=useState("");
  const ratio=data.coverage.total?data.coverage.current/data.coverage.total:0;
  const accounts=data.accounts.filter(a=>`${a.label} ${a.account}`.toLowerCase().includes(accountFilter.toLowerCase()));
  const history=data.history.slice(-18);
  const vals=history.map(p=>p.net); const min=Math.min(...vals); const max=Math.max(...vals,min+1);
  const spark=vals.map((v,i)=>`${i?'L':'M'}${i/Math.max(1,vals.length-1)*250},${55-(v-min)/(max-min)*45}`).join(" ");
  const itemLimit=allEvents?undefined:6;
  return <section id="financial-home">
    <div className="h-heading"><div><span className="h-eyebrow">{new Date(data.today+"T12:00:00Z").toLocaleDateString("en-US",{weekday:"long",month:"long",day:"numeric",timeZone:"UTC"}).toUpperCase()}</span><h1>A little clarity.<br/><em>A lot more possibility.</em></h1></div>
      <div className="h-heading-tools"><button className="h-find" onClick={()=>onSearch("")}><span>⌕</span> Find a transaction <kbd>⌘ K</kbd></button></div>
    </div>
    <div className="h-overview">
      <article className="h-balance"><div className="h-balance-top"><span className="h-eyebrow">CASH AFTER RESERVES</span><span className="h-pill">As of {dayLabel(data.today)}</span></div>
        <div className={`h-big-number ${data.available==null?'h-needs-review':''}`}>{data.available==null?'Needs review':money(data.available,cur)}<span>{cur}</span></div>
        <p>{data.available==null?"Some totals are incomplete. Your review queue has the details.":"Money held in cash accounts, less what you have set aside."}</p>
        <div className="h-cash-track"><span style={{width:`${pct(Math.max(0,data.cash-data.reserved),data.cash)}%`}}/><span style={{width:`${pct(data.reserved,data.cash)}%`}}/></div>
        <div className="h-cash-key"><div><i/><span>Cash held</span><b>{money(data.cash,cur)}</b></div><div><i/><span>Reserved</span><b>{money(data.reserved,cur)}</b></div><div><i/><span>Known outflows · 30 days</span><b>{money(data.upcoming_out,cur)}</b></div></div>
        <div className="h-runway" data-runway-status={data.runway.status}>
          <div><span className="h-eyebrow">CASH RUNWAY</span><strong>{data.runway.months==null ? (data.runway.status==="needs_review" ? "Needs review" : "No spending baseline") : <>{data.runway.months.toLocaleString("en-US",{maximumFractionDigits:1})} <small>{data.runway.months===1?'month':'months'}</small></>}</strong></div>
          <div className="h-runway-basis">{data.runway.status==="ready" ? <><b>Without new income</b><span>At {money(data.runway.monthly_spend,cur)} / month in recorded expenses</span>{data.runway.window&&<span>{monthYear(data.runway.window[0])}{data.runway.window[0]!==data.runway.window[1]&&` – ${monthYear(data.runway.window[1])}`}</span>}<span>Cash after reserves. Loan principal payments excluded.</span></> : <p>{data.runway.status==="needs_review" ? "Accounting, pricing, or connection issues prevent a reliable estimate." : "Record a completed month with positive spending to estimate your runway."}</p>}</div>
        </div>
        <div className="h-balance-foot"><span>◉</span> {data.coverage.current} of {data.coverage.total} accounts have recent coverage evidence <a href="#home-accounts">Review coverage ↗</a></div>
      </article>
      <article className="h-worth"><span className="h-eyebrow">THE BIGGER PICTURE</span><span className="h-worth-label">Net worth {data.unpriced.length>0&&<span className="h-pill">Partial</span>}</span><strong>{money(data.net_worth,cur)}</strong>
        <svg viewBox="0 0 250 65" role="img" aria-label="Recorded net worth history"><path d={spark} fill="none" stroke="var(--market)" strokeWidth="2"/><path d={`${spark} L250,65 L0,65Z`} fill="var(--market)" opacity=".06"/></svg>
        <div className="h-worth-split"><div><span>Assets</span><b>{money(data.assets,cur)}</b></div><div><span>Liabilities</span><b>{money(data.owed,cur)}</b></div></div>
        <a href="/reports">Explore your reports <span>↗</span></a>
      </article>
    </div>
    <div className="h-main-grid"><div className="h-left-stack"><Trajectory data={data} cur={cur}/>      <article className="h-card h-movements"><header className="h-card-head"><div><span className="h-eyebrow">MONEY IN MOTION</span><h2>What’s coming up</h2></div><span className="h-pill">Next 90 days</span></header>
        <div className="h-tabs"><button aria-pressed={eventTab==="next"} onClick={()=>setEventTab("next")}>Timeline <span>{data.events.length}</span></button><button aria-pressed={eventTab==="recurring"} onClick={()=>setEventTab("recurring")}>Recurring <span>{data.recurring.length}</span></button></div>
        {eventTab==="next"?<div>{data.events.slice(0,itemLimit).map((e,i)=><button className="h-movement" key={i} onClick={()=>onSearch(e.label)}><span className="h-date-tile"><b>{e.date.slice(8)}</b>{dayLabel(e.date).split(" ")[0]}</span><span className={`h-flow-icon ${e.amount>0?'incoming':''}`}>{e.amount>0?'↙':'↗'}</span><span className="h-movement-label"><b>{e.label}</b><small>{e.kind==="scheduled"?"Recorded in ledger":"Estimated from history"} · {shortAccount(e.account)}</small></span><strong className={e.amount>0?'h-green':''}>{e.amount>0?'+':''}{fmt(e.amount,cur)}</strong></button>)}{!data.events.length&&<div className="h-empty"><strong>No upcoming movements recorded</strong><p>Add future-dated entries to your ledger to include expected income and bills.</p></div>}</div>
        :<div>{data.recurring.slice(0,itemLimit).map((r,i)=><button className="h-movement" key={i} onClick={()=>onSearch(`${r.label} ${r.account}`)}><span className={`h-flow-icon ${r.active?'':'quiet'}`}>↻</span><span className="h-movement-label"><b>{r.label}</b><small>{r.cadence} · {r.active?'active':'no recent charge'}{r.change&&` · ${r.change.annual>0?'+':''}${fmt(r.change.annual,cur,0)}/year`}</small></span><strong>{fmt(r.amount,cur)}</strong></button>)}{!data.recurring.length&&<div className="h-empty">Not enough repeated charges to identify a pattern yet.</div>}</div>}
        {(eventTab==="next"?data.events:data.recurring).length>6&&<button className="h-more" onClick={()=>setAllEvents(!allEvents)}>{allEvents?'Show less':'Show all movements'} ↓</button>}
      </article></div><div className="h-right-stack"><AttentionPanel items={data.attention} onSearch={onSearch}/>      <article className="h-card h-goals"><header className="h-card-head"><div><span className="h-eyebrow">ROOM FOR YOUR PLANS</span><h2>Something to look forward to</h2></div><span className="h-goal-star">✳</span></header>
        {data.goals.map(g=><a className="h-goal" key={g.account} href={accountUrl(g.account,data.today)} onClick={e=>{if(e.metaKey||e.ctrlKey||e.shiftKey||e.altKey)return;e.preventDefault();onAccount(g.account)}}><div><b>{g.label}</b><span>{g.date?`By ${dayLabel(g.date)}`:'Your own pace'}</span></div><strong>{money(g.funded,cur)} <small>of {money(g.target,cur)}</small></strong><div className="h-goal-track"><span style={{width:`${pct(g.funded??0,g.target)}%`}}/></div><div className="h-goal-foot"><span>{Math.round(pct(g.funded??0,g.target))}% funded</span><span>{g.funded==null?'Balance needs a price':`${money(Math.max(0,g.target-g.funded),cur)} to go`} ↗</span></div></a>)}
        {!data.goals.length&&<div className="h-empty"><span className="h-orbit">✳</span><strong>Give your savings a destination</strong><p>A trip, a cushion, a fresh start. Name a savings account and set its target to see progress here.</p></div>}
        <details className="h-guide"><summary>Set up a goal or reserve <span>+</span></summary><p>Add metadata beneath an account’s existing <code>open</code> directive. A goal tracks progress; a reserve keeps money out of available cash. Amounts use the first operating currency.</p><pre>{'  name: "A little breathing room"\n  liquidity: "cash"\n  reserve: 3000\n  goal: 10000\n  goal-date: "2027-06-01"'}</pre></details>
      </article></div></div>

    <article className="h-card h-accounts" id="home-accounts"><header className="h-card-head"><div><span className="h-eyebrow">EVERY ACCOUNT HAS A PLACE</span><h2>Your financial landscape</h2></div><label className="h-account-filter"><span className="sr-only">Filter accounts</span><input placeholder="Find an account…" value={accountFilter} onChange={e=>setAccountFilter(e.target.value)}/></label></header>
      <div className="h-account-header"><span>Account</span><span>Purpose</span><span>Source coverage</span><span>Balance · {cur}</span></div>
      {accounts.slice(0,allAccounts?undefined:8).map(a=><a className="h-account" href={accountUrl(a.account,data.today)} key={a.account} onClick={e=>{if(e.metaKey||e.ctrlKey||e.shiftKey||e.altKey)return;e.preventDefault();onAccount(a.account)}}>
        <span className="h-account-name"><i className={a.cash?'cash':a.account.startsWith('Liabilities:')?'debt':'holding'}>{a.cash?'↙':a.account.startsWith('Liabilities:')?'↗':'◇'}</i><span><b>{a.label}</b><small>{a.native.map(n=>n.currency).join(' / ')}</small></span></span>
        <span className="h-account-purpose">{a.cash?'Cash':a.liquidity==='cash'?'Investment':a.liquidity}<small>{a.declared?'Declared in ledger':'Inferred · review if needed'}</small></span>
        <span className={`h-account-fresh ${a.fresh?'current':''}`}><span>{a.updated?`Through ${dayLabel(a.updated)}`:a.checked?`Assertion ${dayLabel(a.checked)}`:'Not recorded'}</span><small>{a.last_transaction?`Last activity ${dayLabel(a.last_transaction)}`:'No recorded activity'}</small></span>
        <strong className={a.value!=null&&a.value<0?'h-red':''}>{money(a.value,cur)} <span>↗</span></strong>
      </a>)}
      {!accounts.length&&<div className="h-empty">No accounts match this view.</div>}
      {accounts.length>8&&<button className="h-more" onClick={()=>setAllAccounts(!allAccounts)}>{allAccounts?'Show fewer accounts':`Explore all ${accounts.length} accounts`} ↓</button>}
      <details className="h-guide"><summary>What does source coverage mean? <span>+</span></summary><p>Coverage comes from your declared <code>updated: "YYYY-MM-DD"</code> date or a balance assertion. Recent means within seven days. This app reads your ledger; it does not connect to institutions or independently confirm balances. You can declare <code>liquidity: "cash"</code>, <code>"restricted"</code>, <code>"receivable"</code>, or <code>"illiquid"</code> on an account.</p></details>
    </article>
    <footer className="h-footer"><span>YOUR MONEY. YOUR RECORDS. YOUR PACE.</span><span>One local ledger · {data.accounts.length} accounts · {cur}</span></footer>
  </section>;
}

export function Home({cur,revision,unavailable,onAccount,onSearch}:{cur:string;revision:string;unavailable:boolean;onAccount:(a:string)=>void;onSearch:(q:string)=>void}) {
  const [loaded,setLoaded]=useState<{key:string;data:HomeView}|null>(null);
  const [error,setError]=useState<string|null>(null);
  const key=`${cur}:${revision}`;
  useEffect(()=>{let active=true;setError(null);getHome(cur).then(data=>{if(active)setLoaded({key,data})}).catch(e=>{if(active)setError(String(e.message))});return()=>{active=false}},[key]);
  if(error)return <div className="h-empty" role="alert"><strong>Couldn’t load your financial home</strong><p>{error}</p><button onClick={()=>location.reload()}>Try again</button></div>;
  if(loaded?.key!==key)return <ReportsSkeleton/>;
  const data=unavailable?{...loaded.data,available:null,runway:{...loaded.data.runway,months:null,status:"needs_review" as const},forecast:Object.fromEntries(Object.entries(loaded.data.forecast).map(([k,v])=>[k,{...v,balance:null,low:null,points:[]}]))}:loaded.data;
  return <HomeContent key={cur} data={data} cur={cur} onAccount={onAccount} onSearch={onSearch}/>;
}
