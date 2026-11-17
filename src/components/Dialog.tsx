import { useEffect, useRef, type ReactNode } from "react";

interface DialogProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
}

/**
 * A modal dialog: Esc closes it, focus moves into it. Keys pressed inside never reach the
 * panels: a dialog that closes on Enter must not also open the item under the panel cursor.
 */
export function Dialog({ title, onClose, children }: DialogProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Focus moves into the dialog, unless a field inside already took it (autoFocus).
    if (!ref.current?.contains(document.activeElement)) ref.current?.focus();
  }, []);

  return (
    <div className="dialog-backdrop" onMouseDown={onClose}>
      <div
        ref={ref}
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          event.stopPropagation();
          if (event.key === "Escape") {
            event.preventDefault();
            onClose();
          }
        }}
      >
        <h2 className="dialog__title">{title}</h2>
        {children}
      </div>
    </div>
  );
}
