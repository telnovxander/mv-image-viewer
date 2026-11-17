// The translation layer: every text of the interface goes through t('key').
// Languages are the files in locales/ (MV/I18N.md); missing keys fall back to English.
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { DEFAULT_LANGUAGE, LOCALES, pickLanguage } from "./lib/locales";
import { loadSettings } from "./lib/settings";

const resources = Object.fromEntries(Object.entries(LOCALES).map(([code, messages]) => [code, { translation: messages }]));

export const initialLanguage = pickLanguage(loadSettings().language, navigator.languages ?? [navigator.language], Object.keys(LOCALES));

void i18n.use(initReactI18next).init({
  resources,
  lng: initialLanguage,
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: { escapeValue: false },
});

document.documentElement.lang = initialLanguage;
i18n.on("languageChanged", (language) => {
  document.documentElement.lang = language;
});

export default i18n;
