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
      for (const key of ["ivFormatsTitle", "ivFormatsSubtitle", "ivFormatsText"]) expect(file, `${lang} ${key}`).toMatch(new RegExp(`LangString ${key} `));
    }
  });
});
