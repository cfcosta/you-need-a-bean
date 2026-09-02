/**
 * Shapes standing in for content that is on its way.
 *
 * Each of these replaces a line of prose — "crunching the numbers…",
 * "reading the account…" — with a block the size and place of the thing
 * being waited for. The wait then explains itself, and the page does
 * not jump when the real content lands on top of it.
 */

function Bone({ w, h }: { w: string; h: number }) {
  return <span className="sk" style={{ width: w, height: h }} />;
}

/** A card with a head and a body the height of a chart. */
function CardBones({ body }: { body: number }) {
  return (
    <div className="report-card sk-card">
      <div className="card-head">
        <div className="sk-stack">
          <Bone w="128px" h={13} />
          <Bone w="76px" h={10} />
        </div>
        <div className="sk-stack r">
          <Bone w="72px" h={19} />
          <Bone w="44px" h={10} />
        </div>
      </div>
      <Bone w="100%" h={body} />
    </div>
  );
}

/** The four tiles above a month or an account. */
function StripBones() {
  return (
    <div id="strip" aria-hidden="true">
      {[0, 1, 2, 3].map((i) => (
        <div className="tile sk-card" key={i}>
          <Bone w="56px" h={10} />
          <Bone w="88px" h={19} />
          <Bone w="100%" h={4} />
        </div>
      ))}
    </div>
  );
}

export function ReportsSkeleton() {
  return (
    <section id="reports" aria-busy="true" aria-label="Loading reports">
      <CardBones body={96} />
      <CardBones body={152} />
      <CardBones body={120} />
    </section>
  );
}

export function LiabilitiesSkeleton() {
  return (
    <section id="reports" aria-busy="true" aria-label="Loading liabilities">
      <CardBones body={96} />
      <div className="report-grid">
        <div className="report-col">
          <CardBones body={190} />
        </div>
        <div className="report-col">
          <CardBones body={150} />
        </div>
      </div>
    </section>
  );
}

export function AccountSkeleton() {
  return (
    <div aria-busy="true" aria-label="Loading account">
      <div className="acct-head sk-stack">
        <Bone w="104px" h={10} />
        <Bone w="212px" h={20} />
        <Bone w="150px" h={10} />
      </div>
      <StripBones />
      <div id="account">
        <CardBones body={168} />
        <CardBones body={200} />
      </div>
    </div>
  );
}

/** Before there is a ledger there is no layout to stand in for, so the
 * boot wait is the mark itself, breathing. */
export function BootSkeleton() {
  return (
    <div className="boot" role="status" aria-label="Loading ledger">
      <svg width="44" height="44" viewBox="0 0 32 32" aria-hidden="true">
        <ellipse
          cx="16"
          cy="16"
          rx="9.5"
          ry="12.5"
          transform="rotate(-22 16 16)"
          fill="var(--bean)"
        />
        <path
          d="M11.5 6.5 C 19.5 12.5, 12.5 19.5, 20.5 25.5"
          stroke="var(--bg)"
          strokeWidth="2.8"
          fill="none"
          strokeLinecap="round"
        />
      </svg>
    </div>
  );
}

/** The server is not answering. The one state that still needs words,
 * because nothing about a blank page says why it is blank. */
export function Fatal({ why }: { why: string }) {
  return (
    <div className="boot fatal" role="alert">
      <svg
        width="26"
        height="26"
        viewBox="0 0 16 16"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M8 1.4 15.2 14H.8L8 1.4zM7.25 6v4h1.5V6h-1.5zm0 5v1.5h1.5V11h-1.5z" />
      </svg>
      <b>Cannot reach the ledger server</b>
      <code className="mono">{why}</code>
    </div>
  );
}
