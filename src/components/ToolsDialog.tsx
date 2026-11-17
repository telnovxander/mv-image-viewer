import { useTranslation } from "react-i18next";
import { Dialog } from "./Dialog";

/** The Vollmond "Tools" window: a placeholder until IV-13 fills it (dates, duplicates, batch). */
export function ToolsDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Dialog title={t("tools.title")} onClose={onClose}>
      <p className="dialog__text">{t("tools.placeholder")}</p>
      <div className="dialog__actions">
        <button type="button" className="button button--primary" onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </Dialog>
  );
}
