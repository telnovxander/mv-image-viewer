import { useTranslation } from "react-i18next";
import type { OrientationPlan } from "../lib/backend";
import { Dialog } from "./Dialog";

interface WriteRotationDialogProps {
  plan: OrientationPlan;
  name: string;
  onApply: () => void;
  onClose: () => void;
}

/** Ctrl+S: the preview "from → to" before the rotation is written into the file (safety point 6). */
export function WriteRotationDialog({ plan, name, onApply, onClose }: WriteRotationDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog title={t("write_rotation.title")} onClose={onClose}>
      <p className="dialog__text">{t("write_rotation.files", { count: 1 })}</p>
      <table className="plan">
        <thead>
          <tr>
            <th>{t("write_rotation.file")}</th>
            <th>{t("write_rotation.from")}</th>
            <th>{t("write_rotation.to")}</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>{name}</td>
            <td>{t(`orientation.${plan.from}`)}</td>
            <td>{t(`orientation.${plan.to}`)}</td>
          </tr>
        </tbody>
      </table>
      <p className="dialog__text help__muted">{t("write_rotation.safe")}</p>
      <div className="dialog__actions">
        <button type="button" className="button" autoFocus onClick={onClose}>
          {t("common.cancel")}
        </button>
        <button type="button" className="button button--primary" onClick={onApply}>
          {t("write_rotation.apply")}
        </button>
      </div>
    </Dialog>
  );
}
