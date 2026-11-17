import { useTranslation } from "react-i18next";
import icon from "../../assets/brand/icon.svg";
import { Dialog } from "./Dialog";

// Lumi appears here once assets/brand/lumi-about.png exists (MV/BRAND.md); until then only the icon.
export function AboutDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Dialog title={t("about.title")} onClose={onClose}>
      <div className="about">
        <img className="about__icon" src={icon} alt="" width={64} height={64} />
        <div>
          <p className="about__name">Spiegelmond</p>
          <p className="about__version">{t("about.version", { version: __APP_VERSION__ })}</p>
          <p className="about__family">Moonshine Vibecoder</p>
        </div>
      </div>
      <div className="dialog__actions">
        <button type="button" className="button button--primary" onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </Dialog>
  );
}
