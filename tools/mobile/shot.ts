/**
 * A screenshot of one page at one width, for judging what a probe can
 * only count. Writes a PNG and prints where it went.
 *
 *   bun tools/mobile/shot.ts <out-name> <path> [width] [height] [scheme]
 *
 * OUT_DIR picks the folder (default output/chrome-cdp), CDP_PORT the
 * browser (default 9333), BASE the server (default localhost:2380).
 */
const [name, path, w = "390", h = "1500", scheme = "dark"] = process.argv.slice(2);
const OUT = process.env.OUT_DIR ?? "output/chrome-cdp";
const BASE = process.env.BASE ?? "http://localhost:2380";
const CDP = process.env.CDP_PORT ?? "9333";

const target = await (
  await fetch(`http://localhost:${CDP}/json/new?about:blank`, { method: "PUT" })
).json();
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
ws.onmessage = (e) => {
  const m = JSON.parse(String(e.data));
  if (m.id != null && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
};
const send = (method, params = {}) =>
  new Promise((res) => { const i = ++id; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
const ev = async (e) =>
  (await send("Runtime.evaluate", { expression: e, returnByValue: true, awaitPromise: true }))
    .result?.result?.value;

await send("Page.enable");
await send("Runtime.enable");
await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
await send("Emulation.setDeviceMetricsOverride", {
  width: +w, height: +h, deviceScaleFactor: 1, mobile: +w < 500,
});
await send("Page.navigate", { url: BASE + path });
for (let i = 0; i < 120; i++) {
  await new Promise((r) => setTimeout(r, 150));
  if (!(await ev(`!!document.querySelector('[aria-busy="true"], .boot')`)) && i > 3) break;
}
await new Promise((r) => setTimeout(r, 600));
const shot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
const file = `${OUT}/${name}.png`;
await Bun.write(file, Buffer.from(shot.result.data, "base64"));
console.log(file);
ws.close();
process.exit(0);
