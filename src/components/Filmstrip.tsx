import { ImageOff } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { thumbUrl, type FolderEntry } from "../lib/backend";
import { CELL, centeredScroll, visibleRange } from "../lib/strip";

interface FilmstripProps {
  entries: FolderEntry[];
  index: number;
  onChoose: (index: number) => void;
}

/** One thumbnail; a file that cannot be shown gets a placeholder (a failed request is retried
 * when the cell comes back into view). */
function Thumb({ entry }: { entry: FolderEntry }) {
  const [failed, setFailed] = useState(false);
  return failed ? (
    <ImageOff size={20} aria-hidden="true" className="filmstrip__missing" />
  ) : (
    <img src={thumbUrl(entry.path)} alt="" loading="lazy" decoding="async" draggable={false} onError={() => setFailed(true)} />
  );
}

/** T: the thumbnails of the folder along the bottom, the current one lit and in the middle. */
export function Filmstrip({ entries, index, onChoose }: FilmstripProps) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ left: 0, width: 0 });

  useLayoutEffect(() => {
    const strip = ref.current;
    if (!strip) return;
    const measure = () => setView({ left: strip.scrollLeft, width: strip.clientWidth });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(strip);
    return () => observer.disconnect();
  }, []);

  // The current image moves to the middle.
  useEffect(() => {
    const strip = ref.current;
    if (!strip) return;
    strip.scrollTo({ left: centeredScroll(index, strip.clientWidth, entries.length), behavior: "instant" });
  }, [index, entries.length]);

  const [first, last] = visibleRange(view.left, view.width, entries.length);
  const cells = [];
  for (let i = first; i < last; i++) {
    const entry = entries[i];
    cells.push(
      <button
        key={entry.path}
        type="button"
        className={`filmstrip__cell${i === index ? " filmstrip__cell--current" : ""}`}
        style={{ transform: `translateX(${i * CELL}px)` }}
        title={entry.name}
        aria-label={entry.name}
        aria-current={i === index}
        tabIndex={-1}
        onClick={() => onChoose(i)}
      >
        <Thumb entry={entry} />
      </button>,
    );
  }

  return (
    <nav
      ref={ref}
      className="filmstrip"
      aria-label={t("strip.label")}
      onScroll={(event) => setView({ left: event.currentTarget.scrollLeft, width: event.currentTarget.clientWidth })}
      onWheel={(event) => {
        // The wheel scrolls the strip sideways; it does not page through the images.
        event.stopPropagation();
        if (event.deltaY !== 0) event.currentTarget.scrollLeft += event.deltaY;
      }}
    >
      <div className="filmstrip__track" style={{ width: entries.length * CELL }}>
        {cells}
      </div>
    </nav>
  );
}
