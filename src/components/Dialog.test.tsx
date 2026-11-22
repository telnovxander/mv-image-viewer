// IV-20: every dialog has a cross in the top right corner that closes it; "About" names the
// author and the version and nothing of the family or the tools it was made with.
import { renderToStaticMarkup } from "react-dom/server";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { beforeAll, describe, expect, it } from "vitest";
import { LOCALES } from "../lib/locales";
import { AboutDialog } from "./AboutDialog";
import { Dialog } from "./Dialog";
import { HELP_ARTICLES, HELP_ITEMS } from "./HelpMenu";

beforeAll(async () => {
  await i18n.use(initReactI18next).init({
    resources: Object.fromEntries(Object.entries(LOCALES).map(([code, messages]) => [code, { translation: messages }])),
    lng: "en",
    fallbackLng: "en",
    interpolation: { escapeValue: false },
  });
});

describe("Dialog", () => {
  it("has a close button with a translated label in its header", () => {
    const html = renderToStaticMarkup(
      <Dialog title="Search" onClose={() => undefined}>
        <p>body</p>
      </Dialog>,
    );
    expect(html).toMatch(/<div class="dialog__header"><h2 class="dialog__title">Search<\/h2><button[^>]*class="icon-button dialog__close"/);
    expect(html).toContain(`aria-label="${i18n.t("common.close_window")}"`);
  });

});

describe("About (IV-20)", () => {
  it("shows the author, the version with its date and no family or tool names", async () => {
    for (const code of Object.keys(LOCALES)) {
      await i18n.changeLanguage(code);
      const html = renderToStaticMarkup(<AboutDialog onClose={() => undefined} />);
      expect(html).toContain("Alex Telnov");
      expect(html).toContain(__APP_VERSION__);
      expect(html).not.toMatch(/Moonshine|Vibecoder|Claude/i);
    }
    await i18n.changeLanguage("en");
  });

  it("has a version date in package.json", () => {
    expect(__APP_DATE__).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});

describe("help menu (IV-20)", () => {
  it("has its items, each with a title and an article in every language", () => {
    expect(HELP_ITEMS).toEqual(["about", "viewing", "modes", "vollmond", "hotkeys"]);
    for (const [code, messages] of Object.entries(LOCALES)) {
      const guide = messages.guide as Record<string, { title?: string; lines?: string[] }>;
      for (const item of HELP_ITEMS) expect((messages.guide as Record<string, string>)[`menu_${item}`], `${code} menu_${item}`).toBeTruthy();
      for (const article of HELP_ARTICLES) {
        expect(guide[article]?.title, `${code} ${article}`).toBeTruthy();
        expect(guide[article]?.lines?.length, `${code} ${article}`).toBeGreaterThan(8);
        expect(JSON.stringify(guide[article]), `${code} ${article}`).not.toMatch(/Moonshine|Claude/);
      }
    }
  });
});
