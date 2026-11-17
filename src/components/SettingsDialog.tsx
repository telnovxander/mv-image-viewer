import { useTranslation } from "react-i18next";
import { BACKGROUNDS, SORT_ORDERS, type Background, type SortOrder, type ThemePreference } from "../lib/settings";
import { Dialog } from "./Dialog";
import { LanguageSelect } from "./LanguageSelect";

const THEMES: readonly ThemePreference[] = ["system", "light", "dark"];

interface SettingsDialogProps {
  theme: ThemePreference;
  background: Background;
  sort: SortOrder;
  onSortChange: (sort: SortOrder) => void;
  slideSeconds: number;
  onSlideSecondsChange: (seconds: number) => void;
  onThemeChange: (theme: ThemePreference) => void;
  onBackgroundChange: (background: Background) => void;
  onLanguage: (code: string) => void;
  onAbout: () => void;
  onClose: () => void;
}

function Segmented<T extends string>({ name, values, value, label, onChange }: { name: string; values: readonly T[]; value: T; label: (v: T) => string; onChange: (v: T) => void }) {
  return (
    <div className="segmented">
      {values.map((v) => (
        <label key={v} className={v === value ? "segmented__item segmented__item--on" : "segmented__item"}>
          <input type="radio" name={name} value={v} checked={v === value} onChange={() => onChange(v)} />
          {label(v)}
        </label>
      ))}
    </div>
  );
}

export function SettingsDialog({ theme, background, sort, onSortChange, slideSeconds, onSlideSecondsChange, onThemeChange, onBackgroundChange, onLanguage, onAbout, onClose }: SettingsDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog title={t("settings.title")} onClose={onClose}>
      <h3 className="dialog__section">{t("settings.general")}</h3>
      <fieldset className="field">
        <legend>{t("settings.theme")}</legend>
        <Segmented name="theme" values={THEMES} value={theme} label={(v) => t(`settings.theme_${v}`)} onChange={onThemeChange} />
      </fieldset>
      <fieldset className="field">
        <legend>{t("settings.background")}</legend>
        <Segmented name="background" values={BACKGROUNDS} value={background} label={(v) => t(`settings.background_${v}`)} onChange={onBackgroundChange} />
      </fieldset>
      <fieldset className="field">
        <legend>{t("settings.sort")}</legend>
        <Segmented name="sort" values={SORT_ORDERS} value={sort} label={(v) => t(`settings.sort_${v}`)} onChange={onSortChange} />
      </fieldset>
      <label className="field field--row">
        <span>{t("settings.slide_seconds")}</span>
        <input
          className="select number-input"
          type="number"
          min={1}
          max={60}
          value={slideSeconds}
          onChange={(event) => {
            const value = Number(event.target.value);
            if (Number.isFinite(value) && value >= 1 && value <= 60) onSlideSecondsChange(Math.round(value));
          }}
        />
      </label>
      <label className="field field--row">
        <span>{t("settings.language")}</span>
        <LanguageSelect onChange={onLanguage} />
      </label>
      <div className="dialog__actions">
        <button type="button" className="button" onClick={onAbout}>
          {t("about.title")}
        </button>
        <button type="button" className="button button--primary" onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </Dialog>
  );
}
