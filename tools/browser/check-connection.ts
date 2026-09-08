/**
 * Verify interrupted polling and recovery against a served ledger.
 * Requires an isolated Chromium session with remote debugging enabled.
 *
 * CDP_PORT=9333 BASE=http://127.0.0.1:2326/ bun tools/browser/check-connection.ts
 */
const target = await (
  await fetch(`http://localhost:${process.env.CDP_PORT ?? "9333"}/json/new?about:blank`, {
    method: "PUT",
  })
).json();
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve) => { ws.onopen = resolve; });
let id = 0;
const pending = new Map<number, (message: any) => void>();
ws.onmessage = (event) => {
  const message = JSON.parse(String(event.data));
  if (message.id != null) {
    pending.get(message.id)?.(message);
    pending.delete(message.id);
  }
};
const send = (method: string, params: object = {}) => new Promise<any>((resolve) => {
  pending.set(++id, resolve);
  ws.send(JSON.stringify({ id, method, params }));
});
const evaluate = async (expression: string) => (
  await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true })
).result?.result?.value;
const until = async (expression: string) => {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (await evaluate(expression)) return;
    await Bun.sleep(100);
  }
  throw new Error(`Timed out waiting for: ${expression}`);
};

await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", {
  width: 1512, height: 1100, deviceScaleFactor: 1, mobile: false,
});
await send("Page.navigate", { url: process.env.BASE ?? "http://127.0.0.1:2326/" });
await until('!!document.querySelector("#financial-home")');
await send("Network.enable");
try {
  await send("Network.setBlockedURLs", { urls: ["*api/summary*"] });
  await until('document.body.textContent.includes("Connection lost")');
  if (!await evaluate('document.querySelector("#extra-spending")?.disabled')) {
    throw new Error("Forecast remains actionable while disconnected");
  }
  await send("Network.setBlockedURLs", { urls: [] });
  await until('!document.body.textContent.includes("Connection lost")');
  console.log("PASS: interrupted connection pauses planning and recovers");
} finally {
  await send("Network.setBlockedURLs", { urls: [] });
  ws.close();
}
