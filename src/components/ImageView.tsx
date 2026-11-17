import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ARROW_STEP,
  canPan,
  KEY_STEP,
  PIXELATED_FROM,
  pan,
  resize,
  toggleAt,
  viewFor,
  wheelFactor,
  zoomAt,
  type Size,
  type View,
} from "../lib/zoom";

interface ImageViewProps {
  src: string;
  /** A plain wheel turn while the image is fitted: previous (-1) or next (1) image. */
  onWheelPage: (direction: 1 | -1) => void;
  alt: string;
  onError: () => void;
  onLoad: () => void;
}

/** How long the zoom indicator stays after a change. */
const INDICATOR_MS = 1000;

/**
 * The image with zoom and pan (IV-02): fit by default, 100 % (1), width (W), Ctrl+0 fit,
 * Ctrl+wheel or a touchpad pinch to the cursor, Ctrl+plus / minus to the centre, drag or arrows to
 * move an enlarged image, double click fit ↔ 100 % at the click.
 */
export function ImageView({ src, onWheelPage, alt, onError, onLoad }: ImageViewProps) {
  const { i18n } = useTranslation();
  const stage = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState<Size>({ width: 0, height: 0 });
  const [image, setImage] = useState<Size | null>(null);
  const [view, setView] = useState<View | null>(null);
  const [indicator, setIndicator] = useState(false);
  const indicatorTimer = useRef<number | undefined>(undefined);
  const drag = useRef<{ x: number; y: number; id: number } | null>(null);

  const showIndicator = useCallback(() => {
    setIndicator(true);
    window.clearTimeout(indicatorTimer.current);
    indicatorTimer.current = window.setTimeout(() => setIndicator(false), INDICATOR_MS);
  }, []);
  useEffect(() => () => window.clearTimeout(indicatorTimer.current), []);

  // The window size: fit and width follow it.
  useLayoutEffect(() => {
    const element = stage.current;
    if (!element) return;
    const measure = () => setViewport({ width: element.clientWidth, height: element.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    setImage(null);
    setView(null);
  }, [src]);

  useEffect(() => {
    if (image && viewport.width > 0) setView((v) => (v ? resize(v, image, viewport) : viewFor("fit", image, viewport)));
  }, [image, viewport]);

  const change = useCallback(
    (next: (v: View, img: Size) => View, indicate = true) => {
      if (!image) return;
      setView((v) => (v ? next(v, image) : v));
      if (indicate) showIndicator();
    },
    [image, showIndicator],
  );

  const centre = useCallback(() => ({ x: viewport.width / 2, y: viewport.height / 2 }), [viewport]);

  // Ctrl+wheel and the pinch (Chromium reports it as Ctrl+wheel) zoom to the cursor. The listener
  // is not passive, so the WebView does not zoom the whole page. A plain wheel is left to IV-03.
  useEffect(() => {
    const element = stage.current;
    if (!element) return;
    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey) {
        // A plain wheel browses the folder while the image is fitted (IV-03).
        if (view?.mode === "fit" && event.deltaY !== 0) {
          event.preventDefault();
          onWheelPage(event.deltaY > 0 ? 1 : -1);
        }
        return;
      }
      event.preventDefault();
      const rect = element.getBoundingClientRect();
      const at = { x: event.clientX - rect.left, y: event.clientY - rect.top };
      change((v, img) => zoomAt(v, v.scale * wheelFactor(event.deltaY), at, img, viewport));
    };
    element.addEventListener("wheel", onWheel, { passive: false });
    return () => element.removeEventListener("wheel", onWheel);
  }, [change, viewport, view?.mode, onWheelPage]);

  useEffect(() => {
    // Capture phase: this runs before the browsing keys of App, so arrows move an enlarged image
    // instead of turning to the next one (App skips keys whose default was prevented).
    const onKey = (event: KeyboardEvent) => {
      if (event.altKey || event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement) return;
      if (event.target instanceof Element && event.target.closest("[role=dialog]")) return;
      const key = event.key;
      let handled = true;
      if (event.ctrlKey && (key === "0" || key === ")")) change((_, img) => viewFor("fit", img, viewport));
      else if (event.ctrlKey && (key === "+" || key === "=")) change((v, img) => zoomAt(v, v.scale * KEY_STEP, centre(), img, viewport));
      else if (event.ctrlKey && (key === "-" || key === "_")) change((v, img) => zoomAt(v, v.scale / KEY_STEP, centre(), img, viewport));
      else if (event.ctrlKey) handled = false;
      else if (key === "1") change((v, img) => ({ ...zoomAt(v, 1, centre(), img, viewport), mode: "actual" }));
      else if (key === "w" || key === "W") change((_, img) => viewFor("width", img, viewport));
      else if (key.startsWith("Arrow") && view && image && canPan(view, image, viewport)) {
        const dx = key === "ArrowLeft" ? ARROW_STEP : key === "ArrowRight" ? -ARROW_STEP : 0;
        const dy = key === "ArrowUp" ? ARROW_STEP : key === "ArrowDown" ? -ARROW_STEP : 0;
        change((v, img) => pan(v, dx, dy, img, viewport), false);
      } else handled = false;
      if (handled) event.preventDefault();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [change, centre, image, view, viewport]);

  const point = (event: React.MouseEvent) => {
    const rect = stage.current?.getBoundingClientRect();
    return { x: event.clientX - (rect?.left ?? 0), y: event.clientY - (rect?.top ?? 0) };
  };

  const draggable = view !== null && image !== null && canPan(view, image, viewport);
  const percent = view ? new Intl.NumberFormat(i18n.language, { style: "percent", maximumFractionDigits: view.scale < 0.1 ? 1 : 0 }).format(view.scale) : "";

  return (
    <div
      ref={stage}
      className={draggable ? "image-view image-view--draggable" : "image-view"}
      onDoubleClick={(event) => {
        const at = point(event);
        change((v, img) => toggleAt(v, at, img, viewport));
      }}
      onPointerDown={(event) => {
        if (event.button !== 0 || !draggable) return;
        drag.current = { x: event.clientX, y: event.clientY, id: event.pointerId };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        const start = drag.current;
        if (!start || start.id !== event.pointerId) return;
        const dx = event.clientX - start.x;
        const dy = event.clientY - start.y;
        drag.current = { ...start, x: event.clientX, y: event.clientY };
        change((v, img) => pan(v, dx, dy, img, viewport), false);
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
      onPointerCancel={() => {
        drag.current = null;
      }}
    >
      <img
        className={view && view.scale >= PIXELATED_FROM ? "image-view__image image-view__image--pixelated" : "image-view__image"}
        src={src}
        alt={alt}
        draggable={false}
        style={
          view
            ? { transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})`, visibility: "visible" }
            : { visibility: "hidden" }
        }
        onLoad={(event) => {
          setImage({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight });
          onLoad();
        }}
        onError={onError}
      />
      {indicator && view && (
        <div className="zoom-indicator" role="status">
          {percent}
        </div>
      )}
    </div>
  );
}
