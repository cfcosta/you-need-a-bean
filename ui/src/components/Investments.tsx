import { useEffect, useState } from "react";
import type { AssetClass, Investments, Position, ReportsView } from "../api";
import { getReports } from "../api";
import { fmt, ratio } from "../format";
import { accountUrl, dayLabel } from "../home";
import { holdingPrice, quoteAge, selectHoldings } from "../investments";
import type { HoldingSort } from "../investments";
import { InvestmentPerformance } from "./InvestmentPerformance";
import { ReportsSkeleton } from "./Skeleton";

const units = (n:number) => n.toLocaleString("en-US", {maximumSignificantDigits:8});
const signed = (n:number, cur:string) => `${n>0?"+":""}${fmt(n,cur)}`;
const shade = (index:number) => `var(--alloc-${index%5+1})`;
const className = (c:string|null) => c ?? "Unclassified";

function Allocation({data, selected, onSelect}:{data:Investments;selected:string;onSelect:(s:string)=>void}) {
  let offset = 0;
  const arcs = data.classes.map((c,i) => {
    const share = c.share*100;
    const arc = <circle key={JSON.stringify(c.name)} cx="70" cy="70" r="54" pathLength="100" fill="none" stroke={shade(i)} strokeWidth="17" strokeDasharray={`${share} ${100-share}`} strokeDashoffset={-offset} transform="rotate(-90 70 70)"><title>{className(c.name)} · {ratio(c.share)}</title></circle>;
    offset += share;
    return arc;
  });
  return <article className="i-card i-allocation">
    <header><h2>Allocation</h2><button className="i-text" onClick={()=>onSelect("all")}>Show all</button></header>
    <div className="i-allocation-body">
      <div className="i-ring"><svg viewBox="0 0 140 140" role="img" aria-label="Portfolio allocation by declared asset class"><circle cx="70" cy="70" r="54" fill="none" stroke="var(--line)" strokeWidth="17"/>{arcs}</svg><div><strong>{data.items.length+data.dust.length}</strong><span>holdings</span></div></div>
      <div className="i-legend">{data.classes.map((c:AssetClass,i)=><button key={JSON.stringify(c.name)} aria-pressed={selected===JSON.stringify(c.name)} onClick={()=>onSelect(selected===JSON.stringify(c.name)?"all":JSON.stringify(c.name))}><i style={{background:shade(i)}}/><span>{className(c.name)}</span><b>{ratio(c.share)}</b></button>)}{!data.classes.length&&<p>No priced holdings yet.</p>}</div>
    </div>
  </article>;
}

function Holding({p,cur,today,trusted,onAccount,onSearch}:{p:Position;cur:string;today:string;trusted:boolean;onAccount:(a:string)=>void;onSearch:(q:string)=>void}) {
  const age=quoteAge(p.price_date,today);
  const gain=trusted?p.gain:null;
  return <details className="i-position">
    <summary className="i-position-grid">
      <span className="i-instrument"><i>{p.currency.slice(0,2)}</i><span><b>{p.currency}</b><small>{p.label===p.currency?className(p.class):p.label}</small></span></span>
      <span className="i-quantity"><b>{units(p.units)}</b><small className="i-mobile-label">units</small></span>
      <span className="i-quote"><b>{holdingPrice(p.price,cur)}</b><small className={age!=null&&age>=45?"i-warn":""}>{p.price_date?`${dayLabel(p.price_date)}${age!=null&&age>=45?" · stale":""}`:"No dated quote"}</small></span>
      <span className="i-position-value"><b>{fmt(p.value,cur)}</b><small>{ratio(p.share)} allocation</small></span>
      <span className={`i-position-gain ${gain==null?"":gain>=0?"i-positive":"i-negative"}`}><b>{gain==null?"—":signed(gain,cur)}</b><small>{!trusted?"Needs review":p.basis==null?"Cost incomplete":p.ret==null?"Return unavailable":`${p.ret>0?"+":""}${ratio(p.ret)}`}</small></span>
      <span className="i-chevron" aria-hidden="true">⌄</span>
    </summary>
    <div className="i-position-detail">
      <div className="i-position-facts"><div><span>Average cost</span><b>{trusted&&p.basis!=null?fmt(p.basis,cur):"Unavailable"}</b></div><div><span>First activity</span><b>{p.first}</b></div><div><span>Latest activity</span><b>{p.last}</b></div><div><span>Quote date</span><b>{p.price_date??"Unavailable"}</b></div></div>
      <div className="i-locations"><span className="i-eyebrow">HELD IN {p.locations.length} ACCOUNT{p.locations.length===1?"":"S"}</span>{p.locations.map(a=><a key={a.account} href={accountUrl(a.account,today)} onClick={e=>{if(e.button!==0||e.metaKey||e.ctrlKey||e.shiftKey||e.altKey)return;e.preventDefault();onAccount(a.account)}}><span><b>{a.label}</b><small>{a.account}</small></span><strong>{units(a.units)} {p.currency} <span>↗</span></strong></a>)}</div>
      <footer><p>{p.basis==null?"Some units have no recorded acquisition cost. Missing cost is not treated as zero.":"Cost is estimated by averaging acquisitions within each account. It does not match specific tax lots."}</p><button className="i-text" onClick={()=>onSearch(p.currency)}>Find {p.currency} transactions ↗</button></footer>
    </div>
  </details>;
}

function Portfolio({data,cur,today,revision,unavailable,onAccount,onSearch}:{data:Investments;cur:string;today:string;revision:string;unavailable:boolean;onAccount:(a:string)=>void;onSearch:(q:string)=>void}) {
  const [query,setQuery]=useState("");
  const [assetClass,setClass]=useState("all");
  const [sort,setSort]=useState<HoldingSort>("value");
  const all=[...data.items,...data.dust];
  const shown=selectHoldings(all,query,assetClass,sort);
  const stale=all.filter(p=>(quoteAge(p.price_date,today)??0)>=45);
  const trusted=!unavailable;
  const costKnown=trusted&&data.based_value>0;
  const largest=all.reduce<Position|null>((a,p)=>a==null||p.value>a.value?p:a,null);
  return <section id="investments">
    <div className="i-top-grid">
      <article className="i-card i-overview"><header><h2>Portfolio value</h2><span className="i-badge">{unavailable?"Provisional":data.unpriced.length?"Partial valuation":`As of ${dayLabel(today)}`}</span></header>
        <div className="i-total">{fmt(data.total,cur,0)} <span>{cur}</span></div>
        <div className="i-metrics"><div><span>Unrealized gain (est.)</span><strong className={costKnown?(data.gain>=0?"i-positive":"i-negative"):""}>{costKnown?signed(data.gain,cur):"—"}</strong><small>{unavailable?"Accounting evidence needs review":costKnown&&data.ret!=null?`${data.ret>0?"+":""}${ratio(data.ret)}`:"No complete cost basis"}</small></div><div><span>Value with known cost</span><strong>{trusted&&data.coverage!=null?ratio(data.coverage):"—"}</strong></div><div><span>Largest holding</span><strong>{largest?ratio(largest.share):"—"} <small>{largest?.currency??""}</small></strong></div></div>
        <details className="i-valuation-method"><summary>Valuation &amp; cost basis</summary><p>Values use the latest ledger prices. Unpriced holdings are excluded. Cost coverage is the share of priced portfolio value with a complete recorded basis. Unrealized gains use average acquisition cost; realized gains and investment income are excluded.</p></details>
      </article>
      <Allocation data={data} selected={assetClass} onSelect={setClass}/>
    </div>
    <InvestmentPerformance cur={cur} today={today} revision={revision} unavailable={unavailable}/>
    {(data.unpriced.length>0||stale.length>0||data.unbased_count>0)&&<div className="i-evidence" aria-label="Portfolio data coverage">
      {data.unpriced.length>0&&<div><b>{data.unpriced.length} unpriced holding{data.unpriced.length===1?"":"s"} · excluded from value</b><div>{data.unpriced.map(c=><button key={c} onClick={()=>onSearch(c)}>{c} ↗</button>)}</div></div>}
      {stale.length>0&&<div><b>{stale.length} quote{stale.length===1?"":"s"} 45+ days old</b><button onClick={()=>{setQuery("");setClass("all");setSort("quote")}}>Review oldest prices ↓</button></div>}
      {data.unbased_count>0&&<div><b>{data.unbased_count} holding{data.unbased_count===1?"":"s"} without full cost basis</b><p>{fmt(data.unbased,cur)} excluded from unrealized gains.</p></div>}
    </div>}
    <article className="i-card i-holdings">
      <header><h2>Holdings <span>{all.length}</span></h2><div className="i-controls"><input aria-label="Filter holdings" placeholder="Find a holding or account…" value={query} onChange={e=>setQuery(e.target.value)}/><select aria-label="Asset class" value={assetClass} onChange={e=>setClass(e.target.value)}><option value="all">All asset classes</option>{data.classes.map(c=><option key={JSON.stringify(c.name)} value={JSON.stringify(c.name)}>{className(c.name)}</option>)}</select><select aria-label="Sort holdings" value={sort} onChange={e=>setSort(e.target.value as HoldingSort)}><option value="value">Largest value</option><option value="gain">Highest gain</option><option value="name">Name</option><option value="quote">Oldest quote</option></select></div></header>
      <div className="i-column-labels i-position-grid" aria-hidden="true"><span>Holding</span><span>Quantity</span><span>Price / date</span><span>Market value</span><span>Unrealized gain</span><span/></div>
      {shown.map(p=><Holding key={p.currency} p={p} cur={cur} today={today} trusted={trusted} onAccount={onAccount} onSearch={onSearch}/>)}
      {!shown.length&&<div className="i-empty"><strong>{all.length?"No holdings match":"No priced investments yet"}</strong>{!all.length&&data.unpriced.length>0&&<p>Add prices to value your holdings.</p>}{all.length>0&&<button onClick={()=>{setQuery("");setClass("all")}}>Clear filters</button>}</div>}
      <footer className="i-holdings-foot"><span>{shown.length} of {all.length} holdings</span></footer>
    </article>
    <details className="i-guide"><summary>Ledger setup</summary><p>This page tracks positive commodity balances in asset accounts. Operating currencies and ordinary cash balances are excluded. Allocation percentages use the whole priced portfolio, including small positions. Names and asset classes come from commodity metadata; prices come from dated ledger entries. No live market feed is connected.</p><pre>{'2026-01-01 commodity FUND\n  name: "Broad market fund"\n  asset-class: "Equity funds"\n\n2026-09-08 price FUND 125.50 USD'}</pre><p>Record acquisition costs on postings to support average-cost estimates. Unpriced positions are excluded from the valuation, and missing acquisition costs are not assumed to be zero. These estimates do not calculate tax-lot gains or total investment returns.</p></details>
  </section>;
}

export function InvestmentsPage({cur,today,revision,unavailable,onAccount,onSearch}:{cur:string;today:string;revision:string;unavailable:boolean;onAccount:(a:string)=>void;onSearch:(q:string)=>void}) {
  const [loaded,setLoaded]=useState<{key:string;data:ReportsView}|null>(null);
  const [error,setError]=useState<string|null>(null);
  const [retry,setRetry]=useState(0);
  const key=`${cur}:${today}:${revision}:${retry}`;
  useEffect(()=>{let active=true;setError(null);getReports(6,cur).then(data=>{if(active)setLoaded({key,data})}).catch(e=>{if(active)setError(String(e.message))});return()=>{active=false}},[key]);
  if(error)return <div className="i-empty" role="alert"><strong>Couldn’t load investments</strong><p>{error}</p><button onClick={()=>setRetry(n=>n+1)}>Try again</button></div>;
  if(loaded?.key!==key)return <ReportsSkeleton/>;
  return <Portfolio key={cur} data={loaded.data.investments} cur={cur} today={today} revision={revision} unavailable={unavailable} onAccount={onAccount} onSearch={onSearch}/>;
}
