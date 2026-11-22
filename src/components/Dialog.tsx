import { X } from "lucide-react";
import { useEffect, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

interface DialogProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
  /** A wider window for longer text (the help articles, IV-20). */
  wide?: boolean;
}

/**
 * A modal dialog: Esc and the cross in the top right corner (IV-20, as in Halbmond) close it, focus moves into
 * it. Keys pressed inside never reach the panels: a dialog that closes on Enter must not also
 * open the item under the panel cursor.
 */
export function Dialog({ title, onClose, children, wide }: DialogProps) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Focus moves into the dialog, unless a field inside already took it (autoFocus).
    if (!ref.current?.contains(document.activeElement)) ref.current?.focus();
  }, []);

  return (
    <div className="dialog-backdrop" onMouseDown={onClose}>
      <div
        ref={ref}
        className={wide ? "dialog dialog--wide" : "dialog"}
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
        <div className="dialog__header">
          <h2 className="dialog__title">{title}</h2>
          <button type="button" className="icon-button dialog__close" onClick={onClose} title={t("common.close_window")} aria-label={t("common.close_window")}>
            <X size={16} aria-hidden="true" />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
