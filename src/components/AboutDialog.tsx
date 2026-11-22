import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import icon from "../../assets/brand/icon.svg";
import { dataFolder, revealFile } from "../lib/backend";
import { formatDay } from "../lib/format";
import { Dialog } from "./Dialog";

/** Open-source parts Spiegelmond is built from, with their licences (names are never translated). */
export const COMPONENTS: readonly [string, string][] = [
  ["libheif, libde265", "LGPL-3.0"],
  ["Tauri", "MIT / Apache-2.0"],
  ["React", "MIT"],
  ["i18next, react-i18next", "MIT"],
  ["Lucide", "ISC"],
  ["image, kamadak-exif, trash, serde", "MIT / Apache-2.0"],
  ["SQLite (rusqlite)", "Public Domain / MIT"],
  ["BLAKE3", "CC0-1.0 / Apache-2.0"],
  ["arboard", "MIT / Apache-2.0"],
];

/**
 * "About" (IV-20, MV/STANDARD.md): only in the "?" menu. Name, version and its date from
 * package.json, the author, licences, the data folder.
 */
export function AboutDialog({ onClose }: { onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const language = i18n.resolvedLanguage ?? "en";
  const [folder, setFolder] = useState<string | null>(null);

  useEffect(() => {
    void dataFolder().then(setFolder, () => setFolder(null));
  }, []);

  return (
    <Dialog title={t("about.title")} onClose={onClose}>
      <div className="about">
        <img className="about__icon" src={icon} alt="" width={64} height={64} />
        <div>
          <p className="about__name">Spiegelmond</p>
          <p className="about__version">{t("about.version_date", { version: __APP_VERSION__, date: formatDay(__APP_DATE__, language) })}</p>
          <p className="about__family">{t("about.author", { name: "Alex Telnov" })}</p>
        </div>
      </div>
      <p className="dialog__text about__description">{t("about.description")}</p>
      <h3 className="dialog__section">{t("about.data")}</h3>
      <div className="about__folder">
        <span className="mono">{folder ?? "—"}</span>
        {folder && (
          <button type="button" className="button" onClick={() => void revealFile(folder).catch(() => undefined)}>
            {t("about.open_folder")}
          </button>
        )}
      </div>
      <h3 className="dialog__section">{t("about.licenses")}</h3>
      <p className="dialog__text dialog__muted">{t("about.licenses_text")}</p>
      <table className="about__licenses">
        <tbody>
          {COMPONENTS.map(([name, licence]) => (
            <tr key={name}>
              <td>{name}</td>
              <td className="dialog__muted">{licence}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="dialog__actions">
        <button type="button" className="button button--primary" autoFocus onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </Dialog>
  );
}
