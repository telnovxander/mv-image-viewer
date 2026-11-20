import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "../components/Dialog";
import { pathUrl } from "../lib/backend";
import { FIT, viewAfterKey, type SharedView } from "../lib/compare";

/**
 * IV-18: two frames side by side with one zoom and one position for both: + / − / 0 and the arrows
 * from the keyboard, Ctrl+wheel and dragging with the mouse; Esc closes.
 */
export function CompareDialog({ left, right, onClose }: { left: string; right: string; onClose: () => void }) {
  const { t } = useTranslation();
  const [view, setView] = useState<SharedView>(FIT);
  const drag = useRef<{ x: number; y: number } | null>(null);
  const area = useRef<HTMLDivElement>(null);

  useEffect(() => area.current?.focus(), []);

  const pane = (path: string) => (
    <div className="compare__pane">
      <img
        src={pathUrl(path)}
        alt={path}
        draggable={false}
        style={{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})`, imageRendering: view.scale > 2 ? "pixelated" : "auto" }}
      />
      <span className="compare__name mono">{path.split(/[\\/]/).pop()}</span>
    </div>
  );

  return (
    <Dialog title={t("similar.compare_title")} onClose={onClose}>
      <div
        ref={area}
        className="compare"
        tabIndex={0}
        aria-label={t("similar.compare_keys")}
        onKeyDown={(event) => {
          const next = viewAfterKey(view, event.key);
          if (next) {
            event.preventDefault();
            setView(next);
          }
        }}
        onWheel={(event) => {
          if (!event.ctrlKey) return;
          const next = viewAfterKey(view, event.deltaY < 0 ? "+" : "-");
          if (next) setView(next);
        }}
        onPointerDown={(event) => {
          drag.current = { x: event.clientX, y: event.clientY };
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={(event) => {
          if (!drag.current || view.scale === 1) return;
          const dx = event.clientX - drag.current.x;
          const dy = event.clientY - drag.current.y;
          drag.current = { x: event.clientX, y: event.clientY };
          setView((v) => ({ ...v, x: v.x + dx, y: v.y + dy }));
        }}
        onPointerUp={() => {
          drag.current = null;
        }}
      >
        {pane(left)}
        {pane(right)}
      </div>
      <p className="dialog__text dialog__muted">{t("similar.compare_keys")}</p>
    </Dialog>
  );
}
