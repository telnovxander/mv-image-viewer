import { lazy, Suspense } from "react";
import { useTranslation } from "react-i18next";
import type { Preset } from "../lib/batch";
import { Dialog } from "./Dialog";

// Vollmond's tools are a module of their own, fetched only when the window opens (IV-13): the
// viewer starts as fast as without them.
const ToolsWindow = lazy(() => import("../tools/ToolsWindow"));

/** The Vollmond "Tools" window. */
interface ToolsDialogProps {
  protectedFolders: string[];
  onProtectedChange: (folders: string[]) => void;
  presets: Preset[];
  onPresetsChange: (presets: Preset[]) => void;
  onClose: () => void;
}

export function ToolsDialog({ protectedFolders, onProtectedChange, presets, onPresetsChange, onClose }: ToolsDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog title={t("tools.title")} onClose={onClose}>
      <Suspense fallback={<p className="dialog__text dialog__muted">{t("tools.loading")}</p>}>
        <ToolsWindow protectedFolders={protectedFolders} onProtectedChange={onProtectedChange} presets={presets} onPresetsChange={onPresetsChange} />
      </Suspense>
      <div className="dialog__actions">
        <button type="button" className="button button--primary" onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </Dialog>
  );
}
