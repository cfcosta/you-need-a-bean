/**
 * What a page does wrong at a given viewport width. Drives a headless
 * Chromium over CDP and reports the three ways a narrow layout fails:
 * text that runs past the viewport, text that lands on top of other
 * text, and controls too small to hit with a thumb.
 *
 *   bun tools/mobile/probe.ts <path> [width...]   # e.g. /budget 390 768
 *
 * CDP_PORT picks the browser (default 9333); BASE picks the server
 * (default http://localhost:2380).
 */
import { collide } from "./collide";

const [path, ...widthArgs] = process.argv.slice(2);
const WIDTHS = widthArgs.length ? widthArgs.map(Number) : [390, 768];
const BASE = process.env.BASE ?? "http://localhost:2380";
const CDP = process.env.CDP_PORT ?? "9333";

const target = await (
  await fetch(`http://localhost:${CDP}/json/new?about:blank`, { method: "PUT" })
).json();
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map<number, (v: unknown) => void>();
ws.onmessage = (e) => {
  const m = JSON.parse(String(e.data));
  if (m.id != null && pending.has(m.id)) { pending.get(m.id)!(m); pending.delete(m.id); }
};
const send = (method: string, params: unknown = {}) =>
  new Promise<any>((res) => { const i = ++id; pending.set(i, res as any); ws.send(JSON.stringify({ id: i, method, params })); });
const ev = async (e: string) =>
  (await send("Runtime.evaluate", { expression: e, returnByValue: true, awaitPromise: true }))
    .result?.result?.value;

await send("Page.enable");
await send("Runtime.enable");
await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "dark" }] });

// Runs in the page: leaf text boxes are the unit, because a wrapper
// that contains an overflowing child is not itself the bug.
const AUDIT = `(() => {
  // Lifted in verbatim from collide.ts, where it is unit-tested;
  // the audit runs in the page and cannot import.
  ${collide.toString()}
  const vw = document.documentElement.clientWidth;
  const leaves = [];
  for (const el of document.querySelectorAll('body *')) {
    if (el.closest('#scrim, #toast, svg')) continue;
    // A drawer parked off-canvas is not a layout fault; it is shut.
    if (el.closest('#sidebar:not(.open), #inspector:not(.open)')) continue;
    const cs = getComputedStyle(el);
    if (cs.visibility === 'hidden' || cs.display === 'none' || cs.opacity === '0') continue;
    // A leaf is an element whose own text is not delegated to a child.
    const ownText = [...el.childNodes]
      .filter((n) => n.nodeType === 3 && n.textContent.trim())
      .map((n) => n.textContent.trim()).join(' ');
    if (!ownText) continue;
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) continue;
    // One box per line: text that wraps is not one rectangle.
    leaves.push({ el, r, rects: [...el.getClientRects()], txt: ownText.slice(0, 30) });
  }
  const name = (el) => el.tagName.toLowerCase() +
    (typeof el.className === 'string' && el.className.trim()
      ? '.' + el.className.trim().split(/\\s+/).slice(0, 3).join('.') : '');

  const clipped = [];
  for (const { el, r, txt } of leaves) {
    if (r.right > vw + 1 || r.left < -1) {
      clipped.push({ sel: name(el), txt, left: Math.round(r.left), right: Math.round(r.right) });
    }
  }

  // Two pieces of text sharing pixels. Ancestors legitimately contain
  // descendants, so only compare elements neither of which holds the
  // other, and require a real overlap rather than a shared edge.
  const overlap = [];
  for (let i = 0; i < leaves.length; i++) {
    for (let j = i + 1; j < leaves.length; j++) {
      const a = leaves[i], b = leaves[j];
      if (a.el.contains(b.el) || b.el.contains(a.el)) continue;
      const ax = collide(a.rects, b.rects);
      if (ax > 0) {
        const pa = getComputedStyle(a.el).position, pb = getComputedStyle(b.el).position;
        // Deliberate stacking (a label over a bar) is positioned; flow
        // text that collides is not.
        if (pa !== 'static' || pb !== 'static') continue;
        overlap.push({ a: a.txt, b: b.txt, aSel: name(a.el), bSel: name(b.el), by: ax });
      }
    }
  }

  // Text wider than the box it was given. A grid track squeezed to
  // nothing still paints its text, straight over the neighbour — which
  // is how a table breaks without ever widening the page.
  // Scanned over every box, not just the leaves: a grid track squeezed
  // to nothing usually holds the text in a child, so the element that
  // overflows is the container, not the thing with the words in it.
  const spill = [];
  for (const el of document.querySelectorAll('body *')) {
    if (el.closest('#scrim, #toast, svg')) continue;
    if (el.closest('#sidebar:not(.open), #inspector:not(.open)')) continue;
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden') continue;
    // A box that was asked to scroll is doing as it was told.
    if (cs.overflowX === 'auto' || cs.overflowX === 'scroll') continue;
    const by = el.scrollWidth - el.clientWidth;
    if (by <= 1) continue;
    const txt = (el.textContent || '').trim();
    if (!txt) continue;
    spill.push({ sel: name(el), txt: txt.slice(0, 30), by, box: el.clientWidth });
  }

  const tiny = [];
  for (const el of document.querySelectorAll('button, a[href], [role=button], select, input')) {
    const r = el.getBoundingClientRect();
    if (r.width === 0) continue;
    if (r.height < 32 || r.width < 32) {
      tiny.push({ sel: name(el), w: Math.round(r.width), h: Math.round(r.height),
        txt: (el.textContent || el.getAttribute('aria-label') || '').trim().slice(0, 20) });
    }
  }

  return JSON.stringify({
    vw,
    hScrollPx: document.documentElement.scrollWidth - vw,
    clipped: clipped.slice(0, 15), nClipped: clipped.length,
    spill: spill.slice(0, 15), nSpill: spill.length,
    overlap: overlap.slice(0, 15), nOverlap: overlap.length,
    tiny: tiny.slice(0, 8), nTiny: tiny.length,
  });
})()`;

let bad = false;
for (const w of WIDTHS) {
  await send("Emulation.setDeviceMetricsOverride", { width: w, height: 800, deviceScaleFactor: 1, mobile: w < 500 });
  await send("Page.navigate", { url: BASE + path });
  for (let i = 0; i < 120; i++) {
    await new Promise((r) => setTimeout(r, 150));
    if (!(await ev(`!!document.querySelector('[aria-busy="true"], .boot')`)) && i > 3) break;
  }
  await new Promise((r) => setTimeout(r, 500));
  const out = JSON.parse(await ev(AUDIT));
  if (out.nClipped || out.nOverlap || out.nSpill || out.hScrollPx > 0) bad = true;
  console.log(`--- ${path} @ ${w}px ---`);
  console.log(JSON.stringify(out, null, 1));
}
ws.close();
process.exit(bad ? 1 : 0);
