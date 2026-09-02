import type { ReactNode } from "react";

/** Every card's head: what it is, the span it covers, and the one
 * number it produces.
 *
 * These three used to be a sentence under the title. The sentence
 * spent its first clause restating the title and then buried the span
 * and the total inside prose, which put the two facts a reader
 * actually scans for in the hardest place on the card to find. Here
 * they have fixed positions instead: the span under the name, the
 * number against the right edge, the same on every card.
 */
export function CardHead({
  title,
  span,
  figure,
  note,
}: {
  title: string;
  span?: ReactNode;
  /** The card's headline amount. */
  figure?: ReactNode;
  /** What the figure is measured against, small and beneath it. */
  note?: ReactNode;
}) {
  return (
    <div className="card-head">
      <div>
        <h2>{title}</h2>
        {span != null && <div className="card-span">{span}</div>}
      </div>
      {(figure != null || note != null) && (
        <div className="card-fig">
          {figure != null && <span className="card-total num">{figure}</span>}
          {note != null && <span className="card-note">{note}</span>}
        </div>
      )}
    </div>
  );
}

export interface KeyItem {
  /** The swatch class, which is what the mark on the chart looks
   * like. */
  sw: string;
  label: ReactNode;
  title?: string;
}

/** A chart's key. The swatch is the definition, so the sentence that
 * used to define the series is not needed to read the picture. */
export function Key({ items }: { items: KeyItem[] }) {
  return (
    <div className="key">
      {items.map((k) => (
        <span key={k.sw} className="key-item" title={k.title}>
          <i className={`sw ${k.sw}`} />
          {k.label}
        </span>
      ))}
    </div>
  );
}
