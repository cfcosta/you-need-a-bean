/**
 * Browser acceptance checks for the dedicated investment page.
 * Serve crates/core/tests/fixtures/reports/investments.beancount first.
 * BASE selects that server; CDP_PORT selects an isolated Chromium session.
 */
const BASE=(process.env.BASE ?? "http://127.0.0.1:2380").replace(/\/$/, "");
const CDP_PORT=process.env.CDP_PORT ?? "9333";
const target=await(await fetch(`http://localhost:${CDP_PORT}/json/new?about:blank`,{method:'PUT'})).json();
const ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws.onopen=r);
let id=0;const pending=new Map();ws.onmessage=e=>{const m=JSON.parse(String(e.data));if(m.id){pending.get(m.id)?.(m);pending.delete(m.id)}};
const send=(method,params={})=>new Promise<any>(r=>{pending.set(++id,r);ws.send(JSON.stringify({id,method,params}))});
const ev=async(expression)=>(await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true})).result?.result?.value;
const until=async(expression)=>{for(let i=0;i<100;i++){if(await ev(expression))return;await Bun.sleep(100)}throw new Error(expression)};
try {
 await send('Page.enable');
 await send('Emulation.setDeviceMetricsOverride',{width:1512,height:1100,deviceScaleFactor:1,mobile:false});
 await send('Page.navigate',{url:`${BASE}/investments`});
 await until('document.querySelectorAll(".i-position").length===5');
 if(!await ev('document.querySelector(".i-evidence").textContent.includes("GOLD")'))throw new Error('unpriced holding omitted');
 await ev('[...document.querySelectorAll(".i-legend button")].find(e=>e.textContent.includes("etf")).click()');
 await until('document.querySelectorAll(".i-position").length===1');
 await ev('document.querySelector(".i-allocation .i-text").click()');
 await until('document.querySelectorAll(".i-position").length===5');
 await ev('document.querySelector(".i-controls input").focus()');
 await send('Input.insertText',{text:'Ethereum'});
 await until('document.querySelectorAll(".i-position").length===1');
 await ev('document.querySelector(".i-position summary").click()');
 await ev('document.querySelector(".i-position-detail footer button").click()');
 await until('document.querySelectorAll(".search-result").length===2');
 await send('Input.dispatchKeyEvent',{type:'keyDown',key:'Escape',code:'Escape',windowsVirtualKeyCode:27});
 await until('!document.querySelector("dialog")');
 await ev('document.querySelector(".i-locations a").click()');
 await until('location.pathname.startsWith("/account/")');
 await ev('history.back()');
 await until('!!document.querySelector("#investments")');
 for(const width of [2560,1440,1024,768,390]) {
   await send('Emulation.setDeviceMetricsOverride',{width,height:1100,deviceScaleFactor:1,mobile:width<500});
   await Bun.sleep(250);
   const layout=await ev(`(()=>{const p=document.querySelector('#investments'),m=document.querySelector('#main');return {page:p.getBoundingClientRect().width,main:m.clientWidth,overflow:document.documentElement.scrollWidth-innerWidth,innerOverflow:m.scrollWidth-m.clientWidth}})()`);
   if(Math.abs(layout.page-layout.main)>1||layout.overflow>0||layout.innerOverflow>0)throw new Error(JSON.stringify({width,...layout}));
 }
 await send('Network.enable');await send('Network.setBlockedURLs',{urls:['*api/summary*']});
 await until('document.body.textContent.includes("Connection lost")');
 await until('document.querySelector(".i-badge").textContent==="Provisional"');
 if(!await ev('[...document.querySelectorAll(".i-position-gain b")].every(e=>e.textContent==="—")'))throw new Error('gain estimates survive disconnect');
 await send('Network.setBlockedURLs',{urls:[]});
 await until('!document.body.textContent.includes("Connection lost")');
 console.log('PASS: allocation, filtering, ticker search, account links, Back, five viewport sizes, and disconnected estimates');
} finally {await send('Network.setBlockedURLs',{urls:[]});ws.close()}
