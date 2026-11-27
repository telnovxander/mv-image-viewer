import { describe, expect, it } from "vitest";
import formatsRs from "../../crates/iv-core/src/formats.rs?raw";
import { DECODED_EXTENSIONS, HEIF_EXTENSIONS, isDecoded, isHeif, WEB_EXTENSIONS } from "./formats";

describe("WEB_EXTENSIONS", () => {
  it("is the same list as in iv-core", () => {
    const list = formatsRs.match(/WEB_EXTENSIONS: &\[&str\] = &\[([^\]]*)\]/)?.[1] ?? "";
    expect([...list.matchAll(/"(\w+)"/g)].map((m) => m[1])).toEqual(WEB_EXTENSIONS);
  });
});

describe("DECODED_EXTENSIONS", () => {
  it("are the TIFF, HEIF and RAW lists of iv-core", () => {
    const list = (name: string) => [...(formatsRs.match(new RegExp(`${name}: &\\[&str\\] = &\\[([^\\]]*)\\]`))?.[1] ?? "").matchAll(/"(\w+)"/g)].map((m) => m[1]);
    expect([...list("TIFF_EXTENSIONS"), ...list("HEIF_EXTENSIONS"), ...list("RAW_EXTENSIONS")]).toEqual(DECODED_EXTENSIONS);
    expect(list("HEIF_EXTENSIONS")).toEqual(HEIF_EXTENSIONS);
  });

  it("decide the way an image is served", () => {
    expect(isDecoded("C:\\Fotos\\IMG_1.NEF")).toBe(true);
    expect(isDecoded("scan.tiff")).toBe(true);
    expect(isDecoded("photo.jpg")).toBe(false);
    expect(isDecoded("IMG_0001.HEIC")).toBe(true);
    expect(isHeif("IMG_0001.HEIC")).toBe(true);
    expect(isHeif("scan.tif")).toBe(false);
  });
});

describe("the installer (IV-10)", () => {
  it("registers Open with for exactly the formats Spiegelmond shows", async () => {
    const hooks = (await import("../../src-tauri/nsis/hooks.nsh?raw")).default;
    const block = hooks.match(/!macro IvAssociateAll([\s\S]*?)!macroend/)?.[1] ?? "";
    const registered = [...block.matchAll(/IvFormat \$IvFmt\w+ "(\w+)"/g)].map((m) => m[1]);
    expect(registered).toEqual([...WEB_EXTENSIONS, ...DECODED_EXTENSIONS]);
    const removed = [...(hooks.match(/!macro IvUnassociateAll([\s\S]*?)!macroend/)?.[1] ?? "").matchAll(/IvUnassociateExt "(\w+)"/g)].map((m) => m[1]);
    expect(removed).toEqual(registered);
  });

  it("asks which file types (IV-20): every group has a box on the page, ticked by default, and a /FORMATS name", async () => {
    const hooks = (await import("../../src-tauri/nsis/hooks.nsh?raw")).default;
    const template = (await import("../../src-tauri/nsis/installer.nsi?raw")).default;
    const block = hooks.match(/!macro IvAssociateAll([\s\S]*?)!macroend/)?.[1] ?? "";
    const groups = [...new Set([...block.matchAll(/\$IvFmt(\w+)/g)].map((m) => m[1]))];
    expect(groups).toEqual(["Jpeg", "Png", "Gif", "Webp", "Bmp", "Avif", "Tiff", "Heic", "Raw"]);
    expect(template).toMatch(/Page custom IvFormatsPage IvFormatsLeave/);
    for (const group of groups) {
      expect(hooks, group).toMatch(new RegExp(`Var IvFmt${group}\\b`));
      expect(template, group).toMatch(new RegExp(`Pop \\$IvBox${group}\\s+\\$\\{If\\} \\$IvFmt${group} != 0\\s+\\$\\{NSD_Check\\}`));
      expect(template, group).toMatch(new RegExp(`NSD_GetState\\} \\$IvBox${group} \\$IvFmt${group}`));
      expect(hooks, group).toMatch(new RegExp(`IvFormatFromList \\$IvFmt${group} "${group.toLowerCase()}"`));
    }
    // The page comes before the install itself, and the Welcome page no longer says "Install".
    expect(template.indexOf("Page custom IvFormatsPage")).toBeLessThan(template.indexOf("MUI_PAGE_INSTFILES"));
    expect(template).not.toMatch(/WelcomeInstallButton/);
  });

  it("has the texts of the page in every installer language", async () => {
    for (const lang of ["English", "Russian", "German"]) {
      const file = (await import(`../../src-tauri/nsis/languages/${lang}.nsh?raw`)).default as string;
      for (const key of ["ivFormatsTitle", "ivFormatsSubtitle", "ivFormatsText", "ivMakeDefault"]) expect(file, `${lang} ${key}`).toMatch(new RegExp(`LangString ${key} `));
    }
  });

  it("offers the Default apps page (IV-21): a box on the finish page and a button in the settings, the same page", async () => {
    const hooks = (await import("../../src-tauri/nsis/hooks.nsh?raw")).default;
    const template = (await import("../../src-tauri/nsis/installer.nsi?raw")).default;
    const app = (await import("../../src-tauri/src/lib.rs?raw")).default;
    const uri = hooks.match(/!define IV_DEFAULT_APPS "([^"]+)"/)?.[1] ?? "";
    // Spiegelmond registers for the current user (HKCU), so Settings looks it up as registeredAppUser.
    expect(uri).toBe("ms-settings:defaultapps?registeredAppUser=Spiegelmond");
    expect(app).toContain(`"${uri}"`);
    expect(template).toMatch(/MUI_PAGE_CUSTOMFUNCTION_SHOW IvFinishShow\s+!define MUI_PAGE_CUSTOMFUNCTION_LEAVE IvFinishLeave\s+!insertmacro MUI_PAGE_FINISH/);
    expect(template).toMatch(/ExecShell "open" "\$\{IV_DEFAULT_APPS\}"/);
    // No box when no type was ticked: every group takes part in the check.
    const check = template.match(/\$\{If\} ((?:\$IvFmt\w+?)+) == "(0+)"/);
    expect(check?.[1].split("$").filter(Boolean)).toEqual(["IvFmtJpeg", "IvFmtPng", "IvFmtGif", "IvFmtWebp", "IvFmtBmp", "IvFmtAvif", "IvFmtTiff", "IvFmtHeic", "IvFmtRaw"]);
    expect(check?.[2]).toHaveLength(9);
  });

  it("takes an unowned type at once and gives back only its own default (IV-21)", async () => {
    const hooks = (await import("../../src-tauri/nsis/hooks.nsh?raw")).default;
    const add = hooks.match(/!macro IvAssociateExt EXT([\s\S]*?)!macroend/)?.[1] ?? "";
    expect(add).toMatch(/ReadRegStr \$R2 HKCR "\.\$\{EXT\}" ""\s+\$\{If\} \$R2 == ""\s+\$\{OrIf\} \$R2 == "\$\{IV_PROGID\}"\s+WriteRegStr HKCU "Software\\Classes\\\.\$\{EXT\}" "" "\$\{IV_PROGID\}"/);
    const remove = hooks.match(/!macro IvUnassociateExt EXT([\s\S]*?)!macroend/)?.[1] ?? "";
    expect(remove).toMatch(/ReadRegStr \$R2 HKCU "Software\\Classes\\\.\$\{EXT\}" ""\s+\$\{If\} \$R2 == "\$\{IV_PROGID\}"\s+DeleteRegValue HKCU "Software\\Classes\\\.\$\{EXT\}" ""/);
  });
});
