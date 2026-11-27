import { useEffect, useRef, useState, type RefObject } from "react";

/** How long the mouse rests on a control before its hint shows (IV-24: at most about half a second). */
export const TIP_DELAY_MS = 300;

/**
 * The hint of a control in the Vollmond tools (IV-24): what it does and what it changes. Spread on
 * a button, a checkbox label, a field or a column header: `<button {...tip(t("tips.x"))}>`. The
 * same text goes to `aria-description`, so a screen reader says it too.
 */
export function tip(text: string): Record<string, string> {
  return { "data-tip": text, "aria-description": text };
}

/**
 * The one hint of the whole tools window: it follows the mouse (after TIP_DELAY_MS) and the keyboard
 * focus (at once) over anything inside `root` that has `data-tip`, and stands under that control.
 */
export function TipLayer({ root }: { root: RefObject<HTMLElement | null> }) {
  const [shown, setShown] = useState<{ text: string; x: number; y: number } | null>(null);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    const element = root.current;
    if (!element) return;
    const target = (event: Event) => (event.target instanceof Element ? event.target.closest<HTMLElement>("[data-tip]") : null);
    const show = (control: HTMLElement) => {
      const box = control.getBoundingClientRect();
      const text = control.dataset.tip ?? "";
      if (text) setShown({ text, x: Math.max(8, Math.min(box.left, window.innerWidth - 328)), y: box.bottom + 6 });
    };
    const hide = () => {
      window.clearTimeout(timer.current);
      setShown(null);
    };
    const over = (event: Event) => {
      const control = target(event);
      window.clearTimeout(timer.current);
      if (!control) return setShown(null);
      timer.current = window.setTimeout(() => show(control), TIP_DELAY_MS);
    };
    const focus = (event: Event) => {
      const control = target(event);
      if (control && control instanceof HTMLElement && (event.target as Element).matches(":focus-visible")) show(control);
    };
    element.addEventListener("pointerover", over);
    element.addEventListener("pointerleave", hide);
    element.addEventListener("focusin", focus);
    element.addEventListener("focusout", hide);
    element.addEventListener("pointerdown", hide);
    element.addEventListener("scroll", hide, true);
    return () => {
      window.clearTimeout(timer.current);
      element.removeEventListener("pointerover", over);
      element.removeEventListener("pointerleave", hide);
      element.removeEventListener("focusin", focus);
      element.removeEventListener("focusout", hide);
      element.removeEventListener("pointerdown", hide);
      element.removeEventListener("scroll", hide, true);
    };
  }, [root]);

  if (!shown) return null;
  return (
    <div className="tip" role="tooltip" style={{ left: shown.x, top: shown.y }}>
      {shown.text}
    </div>
  );
}
