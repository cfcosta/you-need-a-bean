import { useEffect, useState } from "react";
import { getInvestmentPerformance } from "../api";
import type { InvestmentPerformance as PerformanceData } from "../api";
import { fmt, ratio } from "../format";
import { performanceStart, performanceChart } from "../investments";
import type { PerformancePeriod } from "../investments";

const periods:PerformancePeriod[]=["1M","3M","6M","YTD","1Y","All"];
const dateLabel=(date:string)=>new Date(date+"T00:00:00Z").toLocaleDateString("en-US",{month:"short",day:"numeric",year:"numeric",timeZone:"UTC"});
const money=(n:number|null,cur:string)=>n==null?"—":fmt(n,cur);
const signed=(n:number|null,cur:string)=>n==null?"—":`${n>0?"+":""}${fmt(n,cur)}`;
const tone=(n:number|null)=>n==null?"":n>=0?"i-positive":"i-negative";

function History({data,cur,trusted}:{data:PerformanceData;cur:string;trusted:boolean}) {
  const chart=performanceChart(data.points,data.opening);
  const [index,setIndex]=useState(data.points.length-1);
  const selected=chart.points[Math.min(index,chart.points.length-1)];
  if(!selected)return null;
  return <div className="i-chart">
    <div className="i-chart-readout" aria-live="polite"><time>{dateLabel(selected.date)}</time><span>Value <b>{money(selected.value,cur)}</b></span><span>Price gain <b className={tone(trusted?selected.gain:null)}>{signed(trusted?selected.gain:null,cur)}</b></span></div>
    <div className="i-chart-plot"><div className="i-chart-scale"><span>{fmt(chart.max,cur,0)}</span><span>{fmt((chart.max+chart.min)/2,cur,0)}</span><span>{fmt(chart.min,cur,0)}</span></div><svg viewBox="0 0 1000 240" preserveAspectRatio="none" role="img" aria-label="Portfolio value compared with opening value plus net flows" onPointerMove={e=>{const bounds=e.currentTarget.getBoundingClientRect();const x=(e.clientX-bounds.left)/bounds.width*1000;let nearest=0;chart.points.forEach((p,i)=>{if(Math.abs(p.x-x)<Math.abs(chart.points[nearest]!.x-x))nearest=i});setIndex(nearest)}}>
      {[20,120,220].map(y=><line key={y} x1="0" x2="1000" y1={y} y2={y} stroke="var(--line)" strokeDasharray="3 5"/>)}
      <path d={chart.capitalPath} fill="none" stroke="var(--ink-2)" strokeWidth="1.5" strokeDasharray="6 5" vectorEffect="non-scaling-stroke"/>
      <path d={chart.valuePath} fill="none" stroke="var(--market)" strokeWidth="3" strokeLinejoin="round" vectorEffect="non-scaling-stroke"/>
      <line x1={selected.x} x2={selected.x} y1="15" y2="225" stroke="var(--line-2)" vectorEffect="non-scaling-stroke"/>
      {selected.y!=null&&<ellipse cx={selected.x} cy={selected.y} rx="4" ry="4" fill="var(--market)"/>}
    </svg></div>
    <input type="range" aria-label="Explore investment history" min="0" max={Math.max(0,chart.points.length-1)} value={index} onChange={e=>setIndex(Number(e.target.value))} aria-valuetext={`${selected.date}: value ${money(selected.value,cur)}, price gain ${signed(trusted?selected.gain:null,cur)}`}/>
    <div className="i-chart-dates"><span>{dateLabel(data.start)}</span><span>{dateLabel(data.end)}</span></div>
    <div className="i-chart-legend"><span><i/>Portfolio value</span><span><i/>Opening value + net flows</span><small>Month-end and activity-day samples · drag or use arrow keys to explore</small></div>
  </div>;
}

function Results({data,cur,unavailable}:{data:PerformanceData;cur:string;unavailable:boolean}) {
  const trusted=!unavailable&&data.trusted;
  const gain=trusted?data.gain:null,ret=trusted?data.ret:null;
  const rows=[...data.holdings].sort((a,b)=>a.gain==null?(b.gain==null?0:1):b.gain==null?-1:b.gain-a.gain);
  return <>
    <div className="i-performance-metrics"><div className="i-performance-lead"><span>Price gain / loss</span><div><strong className={`i-period-gain ${tone(gain)}`}>{signed(gain,cur)}</strong><b className={`i-return ${tone(ret)}`}>{ret==null?"—":`${ret>0?"+":""}${ratio(ret)}`}</b></div><small>{!trusted?"Accounting evidence needs review":data.issues.length?"Historical data is incomplete":ret==null?"No positive weighted capital for a return estimate":"Estimated period return · adjusted for flow timing"}</small></div>
      <div><span>Opening value</span><strong>{money(data.opening,cur)}</strong></div><div><span>Net flows into holdings</span><strong>{signed(data.net_flows,cur)}</strong></div><div><span>Closing value</span><strong>{money(data.closing,cur)}</strong></div>
    </div>
    <History key={`${data.start}:${data.end}`} data={data} cur={cur} trusted={trusted}/>
    {(data.issues.length>0||data.warnings.length>0)&&<details className="i-performance-evidence"><summary>{data.issues.length?`${data.issues.length} historical data gap${data.issues.length===1?"":"s"} · complete return unavailable`:`Older quotes affect ${data.warnings.length} holding${data.warnings.length===1?"":"s"}`}</summary><ul>{[...data.issues,...data.warnings].map(w=><li key={w}>{w}</li>)}</ul></details>}
    {rows.length>0?<div className="i-period-holdings"><table><caption>Performance by holding <span>Includes positions sold during this period</span></caption><thead><tr><th>Holding</th><th>Opening</th><th>Net flows</th><th>Closing</th><th>Price gain</th><th>Est. return</th></tr></thead><tbody>{rows.map(p=><tr key={p.currency}><th scope="row"><b>{p.currency}</b><small>{p.closing===0?"Sold · ":""}{p.label}</small></th><td>{money(p.opening,cur)}</td><td>{signed(p.net_flows,cur)}</td><td>{money(p.closing,cur)}</td><td className={tone(trusted?p.gain:null)}>{signed(trusted?p.gain:null,cur)}</td><td className={tone(trusted?p.ret:null)}>{trusted&&p.ret!=null?`${p.ret>0?"+":""}${ratio(p.ret)}`:"—"}</td></tr>)}</tbody></table></div>:<p className="i-period-empty">No investment positions or activity in this period. Try a longer range.</p>}
    <p className="i-performance-scope">Price performance excludes dividends, interest, fees, and taxes.</p>
    <details className="i-performance-method"><summary>What this performance measures</summary><p>Price gain = closing value − opening value − net flows. Flows include purchases, sales, and new units received as rewards. Transfers between asset accounts cancel out. Execution prices are used when recorded; additions otherwise use acquisition cost, then ledger quotes. Sales without execution prices use ledger quotes.</p><p>The percentage uses Modified Dietz with end-of-day flow timing. It is an estimate for this period, not an annualized rate. Cash dividends, interest, fees, and taxes are excluded. Currency movements are included. This is price performance, not total return.</p><p>Opening and closing values are at the end of the dates shown. Historical quotes are carried forward until a newer ledger quote exists. Lines connect recorded samples; missing valuations appear as gaps.</p></details>
  </>;
}

export function InvestmentPerformance({cur,today,revision,unavailable}:{cur:string;today:string;revision:string;unavailable:boolean}) {
  const [period,setPeriod]=useState<PerformancePeriod>("YTD");
  const [loaded,setLoaded]=useState<{key:string;data:PerformanceData}|null>(null);
  const [error,setError]=useState<{key:string;message:string}|null>(null);
  const [retry,setRetry]=useState(0);
  const start=performanceStart(today,period);
  const key=`${cur}:${today}:${revision}:${period}:${retry}`;
  useEffect(()=>{let active=true;getInvestmentPerformance(cur,start).then(data=>{if(active)setLoaded({key,data})}).catch(e=>{if(active)setError({key,message:String(e.message)})});return()=>{active=false}},[key]);
  return <article className="i-card i-performance" aria-label="Investment performance">
    <header><div><span className="i-eyebrow">PERFORMANCE OVER TIME</span><h2>See what changed</h2></div><div className="i-periods" role="group" aria-label="Performance period">{periods.map(p=><button key={p} aria-pressed={period===p} onClick={()=>setPeriod(p)}>{p}</button>)}</div></header>
    {error?.key===key?<div className="i-empty" role="alert"><strong>Couldn’t load performance</strong><p>{error.message}</p><button onClick={()=>setRetry(n=>n+1)}>Try again</button></div>:loaded?.key===key?<Results data={loaded.data} cur={cur} unavailable={unavailable}/>:<div className="i-performance-loading" aria-busy="true">Reading investment history…</div>}
  </article>;
}
