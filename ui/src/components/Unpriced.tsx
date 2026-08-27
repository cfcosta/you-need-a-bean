/** Currencies the ledger never priced in the display currency.
 *
 * This used to be a paragraph explaining what that does to every figure
 * on the page. The figures already show a dash where they can't be
 * converted, so the banner is down to the codes themselves; the
 * consequence and the fix are on the hover.
 */
export function Unpriced({
  codes,
  cur,
  where,
}: {
  codes: string[];
  cur: string;
  /** What the missing prices are missing from, e.g. "these numbers". */
  where: string;
}) {
  if (codes.length === 0) return null;
  const them = codes.length === 1 ? "it" : "them";
  return (
    <div
      className="notice"
      title={
        `Everything held in ${them}, and the income and spending that ` +
        `passed through ${them}, is left out of ${where}.\n` +
        `Add price directives to bring ${them} in.`
      }
    >
      <svg
        width="13"
        height="13"
        viewBox="0 0 16 16"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M8 1.4 15.2 14H.8L8 1.4zM7.25 6v4h1.5V6h-1.5zm0 5v1.5h1.5V11h-1.5z" />
      </svg>
      {codes.map((c) => (
        <code key={c}>{c}</code>
      ))}
      <span>not priced in {cur}</span>
    </div>
  );
}
