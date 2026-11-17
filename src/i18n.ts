// The translation layer: every text of the interface goes through t('key').
// Languages are the files in locales/ (MV/I18N.md); missing keys fall back to English.
// The saved language choice comes with the settings (IV-12); until then the system language.
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { DEFAULT_LANGUAGE, LOCALES, pickLanguage } from "./lib/locales";

const resources = Object.fromEntries(Object.entries(LOCALES).map(([code, messages]) => [code, { translation: messages }]));

export const initialLanguage = pickLanguage(null, navigator.languages ?? [navigator.language], Object.keys(LOCALES));

void i18n.use(initReactI18next).init({
  resources,
  lng: initialLanguage,
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: { escapeValue: false },
});

document.documentElement.lang = initialLanguage;

export default i18n;
