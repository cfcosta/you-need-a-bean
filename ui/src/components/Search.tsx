import { useEffect, useRef, useState } from "react";
import { searchLedger } from "../api";
import type { SearchTxn } from "../api";
import { fmt } from "../format";
import { dayLabel } from "../home";

export function Search({initial,cur,onClose}:{initial:string;cur:string;onClose:()=>void}) {
  const dialog=useRef<HTMLDialogElement>(null);
  const input=useRef<HTMLInputElement>(null);
  const [q,setQ]=useState(initial);
  const [offset,setOffset]=useState(0);
  const [result,setResult]=useState<{total:number;items:SearchTxn[]}|null>(null);
  const [error,setError]=useState<string|null>(null);
  useEffect(()=>{dialog.current?.showModal();input.current?.focus();},[]);
  useEffect(()=>{let active=true;setResult(null);setError(null);const timer=setTimeout(()=>{searchLedger(q,cur,offset).then(r=>{if(active)setResult(r)}).catch(e=>{if(active)setError(e.message)});},180);return()=>{active=false;clearTimeout(timer)}},[q,cur,offset]);
  return <dialog ref={dialog} className="ledger-search" onCancel={onClose} onClick={e=>{if(e.target===dialog.current)onClose()}} aria-label="Search your ledger">
    <div className="search-input-row"><span>⌕</span><input ref={input} value={q} onChange={e=>{setQ(e.target.value);setOffset(0)}} placeholder="Payee, account, tag, date, or a few words…" aria-label="Search transactions"/><button onClick={onClose} aria-label="Close search">Esc</button></div>
    <div className="search-meta"><span>{result?`${result.total.toLocaleString()} matching transactions`:'Searching your ledger…'}</span><span>Across every account</span></div>
    <div className="search-results" aria-live="polite">{error&&<p role="alert">{error}</p>}{result?.items.map((t,i)=><details key={`${offset}:${i}`} className="search-result"><summary><span className="search-result-date">{dayLabel(t.date)}<small>{t.date.slice(0,4)}</small></span><span className="search-result-label"><b>{t.payee??t.narration??'Transaction'}</b><small>{t.narration??t.account}</small></span>{t.scheduled&&<span className="h-pill">Scheduled</span>}<strong>{t.converted==null?'—':fmt(t.converted,cur)}</strong></summary><div className="search-result-detail"><div className="search-tags">{[...t.tags.map(v=>'#'+v),...t.links.map(v=>'^'+v)].map(tag=><button key={tag} onClick={()=>{setQ(tag);setOffset(0)}}>{tag}</button>)}</div>{t.postings.map((p,j)=><div className="search-posting" key={j}><button onClick={()=>{setQ(p.account);setOffset(0)}}>{p.account}</button><b>{p.amount==null?'—':fmt(p.amount,p.currency??cur)}</b></div>)}{t.documents.map(d=><a className="search-document" href={`/api/document/${d.id}`} target="_blank" rel="noreferrer" key={d.id}>↗ {d.name}</a>)}{t.source&&<code className="search-source">{t.source.path}:{t.source.line}</code>}</div></details>)}{result?.total===0&&<div className="h-empty"><strong>No matches yet</strong><p>Try a merchant, an account name, a tag, or a date.</p></div>}</div>
    <footer><button disabled={!offset} onClick={()=>setOffset(Math.max(0,offset-50))}>← Previous</button><span>{result?.total?`${offset+1}–${offset+result.items.length}`:'Ledger search'}</span><button disabled={!result||offset+50>=result.total} onClick={()=>setOffset(offset+50)}>Next →</button></footer>
  </dialog>;
}
