import { useTranslation } from "react-i18next";
import { HOTKEYS, localizeKeys } from "../lib/hotkeys";
import { Dialog } from "./Dialog";

/** F1 or "?": the keyboard shortcuts of the window (MV/STANDARD.md). */
export function HelpDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Dialog title={t("help.title")} onClose={onClose}>
      <div className="help">
        <table>
          <tbody>
            {HOTKEYS.map((key) => (
              <tr key={key.action} className={key.done ? undefined : "help__coming"}>
                <td className="help__keys">{localizeKeys(key.keys, (name) => t(`keys.${name}`))}</td>
                <td>
                  {t(`hotkeys.${key.action}`)}
                  {!key.done && <span className="help__muted"> · {t("help.coming")}</span>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="dialog__actions">
        <button type="button" className="button button--primary" autoFocus onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </Dialog>
  );
}
