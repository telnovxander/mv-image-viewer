import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { localizeKeys } from "../lib/hotkeys";

export type ImageAction = "copy_image" | "copy_path" | "reveal" | "open_with" | "delete";

const ITEMS: readonly { action: ImageAction; keys: string }[] = [
  { action: "copy_image", keys: "Ctrl+C" },
  { action: "copy_path", keys: "Ctrl+Shift+C" },
  { action: "reveal", keys: "" },
  { action: "open_with", keys: "" },
  { action: "delete", keys: "Delete" },
];

interface ImageMenuProps {
  x: number;
  y: number;
  onChoose: (action: ImageAction) => void;
  onClose: () => void;
}

/** The right-click menu of the image (IV-07): the same actions as the keys. */
export function ImageMenu({ x, y, onChoose, onClose }: ImageMenuProps) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x, y });
  const [focused, setFocused] = useState(0);

  // Keep the menu inside the window.
  useLayoutEffect(() => {
    const box = ref.current?.getBoundingClientRect();
    if (!box) return;
    setPosition({ x: Math.min(x, window.innerWidth - box.width - 4), y: Math.min(y, window.innerHeight - box.height - 4) });
  }, [x, y]);

  useEffect(() => {
    ref.current?.querySelectorAll<HTMLButtonElement>("button")[focused]?.focus();
  }, [focused]);

  useEffect(() => {
    const close = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    window.addEventListener("mousedown", close, true);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("mousedown", close, true);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  const choose = (action: ImageAction) => {
    onClose();
    onChoose(action);
  };

  return (
    <div
      ref={ref}
      className="image-menu"
      role="menu"
      style={{ left: position.x, top: position.y }}
      onKeyDown={(event) => {
        event.stopPropagation();
        event.preventDefault();
        if (event.key === "Escape") onClose();
        else if (event.key === "ArrowDown") setFocused((i) => (i + 1) % ITEMS.length);
        else if (event.key === "ArrowUp") setFocused((i) => (i - 1 + ITEMS.length) % ITEMS.length);
        else if (event.key === "Enter") choose(ITEMS[focused].action);
      }}
      onContextMenu={(event) => event.preventDefault()}
    >
      {ITEMS.map(({ action, keys }) => (
        <button key={action} type="button" role="menuitem" className="image-menu__item" onClick={() => choose(action)}>
          <span>{t(`menu.${action}`)}</span>
          <span className="image-menu__key">{localizeKeys(keys, (name) => t(`keys.${name}`))}</span>
        </button>
      ))}
    </div>
  );
}
