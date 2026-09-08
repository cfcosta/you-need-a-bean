/**
 * Browser acceptance checks for the dedicated investment page.
 * Serve crates/core/tests/fixtures/reports/performance.beancount first.
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
 await send('Page.enable');await send('Network.enable');
 await send('Emulation.setDeviceMetricsOverride',{width:1512,height:1100,deviceScaleFactor:1,mobile:false});
 await send('Page.navigate',{url:`${BASE}/investments`});
 await until('!!document.querySelector(".i-performance .i-period-gain")');
 if(!await ev('document.querySelector(".i-period-gain").textContent.includes("575")'))throw new Error('period gain missing');
 await ev('[...document.querySelectorAll(".i-periods button")].find(e=>e.textContent==="1M").click()');
 await until('document.querySelector(".i-periods [aria-pressed=true]")?.textContent==="1M" && !!document.querySelector(".i-performance .i-period-gain")');
 if(!await ev('document.querySelector(".i-period-gain").textContent.includes("0.00")'))throw new Error('old period data shown');
 await ev('[...document.querySelectorAll(".i-periods button")].find(e=>e.textContent==="All").click()');
 await until('!!document.querySelector(".i-period-holdings")');
 if(!await ev('document.querySelector(".i-period-holdings").textContent.includes("Sold")'))throw new Error('sold position omitted');
 await ev('document.querySelector(".i-chart input").focus()');
 await send('Input.dispatchKeyEvent',{type:'keyDown',key:'Home',code:'Home',windowsVirtualKeyCode:36});
 await until('document.querySelector(".i-chart input").value==="0"');
 for(const width of [2560,1440,1024,768,390]) {
  await send('Emulation.setDeviceMetricsOverride',{width,height:1100,deviceScaleFactor:1,mobile:width<500});await Bun.sleep(150);
  if(await ev('document.documentElement.scrollWidth>innerWidth || document.querySelector("#main").scrollWidth>document.querySelector("#main").clientWidth'))throw new Error(`overflow ${width}`);
 }
 await send('Network.setBlockedURLs',{urls:['*api/investments/performance*']});
 await ev('[...document.querySelectorAll(".i-periods button")].find(e=>e.textContent==="6M").click()');
 await until('!!document.querySelector(".i-performance [role=alert]")');
 await send('Network.setBlockedURLs',{urls:[]});
 await ev('document.querySelector(".i-performance [role=alert] button").click()');
 await until('!!document.querySelector(".i-period-gain")');
 await send('Network.setBlockedURLs',{urls:['*api/summary*']});
 await until('document.body.textContent.includes("Connection lost")');
 await until('document.querySelector(".i-period-gain")?.textContent==="—"');
 console.log('PASS: period changes, sold holdings, keyboard chart, five viewport sizes, retry, and disconnected gains');
} finally {await send('Network.setBlockedURLs',{urls:[]});ws.close()}
