import { useTranslation } from "react-i18next";
import { availableLanguages } from "../lib/locales";

const LANGUAGES = availableLanguages();

/** Language dropdown: each language under its own name; switches at once, no restart. */
export function LanguageSelect({ onChange }: { onChange: (code: string) => void }) {
  const { t, i18n } = useTranslation();
  return (
    <select
      className="select"
      value={i18n.resolvedLanguage}
      aria-label={t("language.select")}
      title={t("language.select")}
      onChange={(event) => onChange(event.target.value)}
    >
      {LANGUAGES.map((language) => (
        <option key={language.code} value={language.code} lang={language.code}>
          {language.name}
        </option>
      ))}
    </select>
  );
}
