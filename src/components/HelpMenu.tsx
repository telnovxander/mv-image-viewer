import { CircleHelp } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export type HelpItem = "about" | "viewing" | "modes" | "vollmond" | "hotkeys";

/** The items of the help menu, in this order (IV-20). */
export const HELP_ITEMS: readonly HelpItem[] = ["about", "viewing", "modes", "vollmond", "hotkeys"];

/** The articles among them (the rest are dialogs of their own). */
export const HELP_ARTICLES = ["viewing", "modes", "vollmond"] as const;

interface HelpMenuProps {
  onChoose: (item: HelpItem) => void;
  /** The menu is open: the bars do not hide meanwhile. */
  onOpenChange: (open: boolean) => void;
}

/**
 * The "?" button of the top bar (IV-20): About, how to view, Sichel and Vollmond, Vollmond step by
 * step, keyboard shortcuts. F1 still opens the keyboard shortcuts directly.
 */
export function HelpMenu({ onChoose, onOpenChange }: HelpMenuProps) {
  const { t } = useTranslation();
  const [open, setOpenState] = useState(false);
  const [focused, setFocused] = useState(0);
  const ref = useRef<HTMLDivElement>(null);
  const setOpen = (next: boolean) => {
    setOpenState(next);
    onOpenChange(next);
  };

  useEffect(() => {
    if (!open) return;
    ref.current?.querySelectorAll<HTMLButtonElement>(".image-menu__item")[focused]?.focus();
  }, [open, focused]);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", close, true);
    return () => window.removeEventListener("mousedown", close, true);
  });

  const choose = (item: HelpItem) => {
    setOpen(false);
    (document.activeElement as HTMLElement | null)?.blur();
    onChoose(item);
  };

  return (
    <div className="help-menu" ref={ref}>
      <button
        type="button"
        className="icon-button"
        aria-haspopup="menu"
        aria-expanded={open}
        title={t("guide.button")}
        aria-label={t("guide.button")}
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => {
          setFocused(0);
          setOpen(!open);
        }}
      >
        <CircleHelp size={16} aria-hidden="true" />
      </button>
      {open && (
        <div
          className="image-menu help-menu__menu"
          role="menu"
          onKeyDown={(event) => {
            event.stopPropagation();
            if (event.key === "Escape") {
              event.preventDefault();
              setOpen(false);
              (document.activeElement as HTMLElement | null)?.blur();
            } else if (event.key === "ArrowDown") {
              event.preventDefault();
              setFocused((i) => (i + 1) % HELP_ITEMS.length);
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              setFocused((i) => (i - 1 + HELP_ITEMS.length) % HELP_ITEMS.length);
            }
          }}
        >
          {HELP_ITEMS.map((item, index) => (
            <button key={item} type="button" role="menuitem" className="image-menu__item" onMouseEnter={() => setFocused(index)} onClick={() => choose(item)}>
              <span>{t(`guide.menu_${item}`)}</span>
              {item === "hotkeys" && <span className="image-menu__key">{"F1"}</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
