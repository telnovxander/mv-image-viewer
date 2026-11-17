import i18n from "i18next";
import { beforeAll, describe, expect, it } from "vitest";
import { exposureText, formatDateTime, formatSize, formatTaken, numberText } from "./format";
import { LOCALES } from "./locales";

beforeAll(async () => {
  await i18n.init({
    resources: Object.fromEntries(Object.entries(LOCALES).map(([code, m]) => [code, { translation: m }])),
    lng: "en",
    fallbackLng: "en",
    interpolation: { escapeValue: false },
  });
});

const NBSP = /[  ]/g;

describe("formatSize", () => {
  it.each([
    ["ru", "1 234,5 КБ"],
    ["de", "1.234,5 KB"],
    ["en", "1,234.5 KB"],
  ])("%s", (language, expected) => {
    const t = i18n.getFixedT(language);
    expect(formatSize(1234.5 * 1024, language, t).replace(NBSP, " ")).toBe(expected);
  });

  it("shows small sizes in bytes and large ones in bigger units", () => {
    const t = i18n.getFixedT("en");
    expect(formatSize(512, "en", t)).toBe("512 B");
    expect(formatSize(5 * 1024 ** 3, "en", t)).toBe("5,120 MB");
    expect(formatSize(3 * 1024 ** 4, "en", t)).toBe("3,072 GB");
  });
});

describe("plural forms", () => {
  it.each([
    ["ru", [1, 2, 5, 21, 1.5, 100_000], ["1 изображение", "2 изображения", "5 изображений", "21 изображение", "1,5 изображения", "100 000 изображений"]],
    ["de", [1, 2, 5], ["1 Bild", "2 Bilder", "5 Bilder"]],
    ["en", [1, 2, 5, 100_000], ["1 image", "2 images", "5 images", "100,000 images"]],
  ] as const)("%s", (language, counts, expected) => {
    const t = i18n.getFixedT(language);
    expect(counts.map((count) => t("images.count", { count }).replace(/[\u00a0\u202f]/g, " "))).toEqual(expected);
  });
});

describe("formatDateTime", () => {
  const ms = Date.UTC(2026, 10, 16, 12, 5);
  it.each([
    ["ru", /16\.11\.2026/],
    ["de", /16\.11\.26/],
    ["en", /11\/16\/26/],
  ])("%s", (language, pattern) => {
    expect(formatDateTime(ms, language)).toMatch(pattern);
  });
});

describe("EXIF values", () => {
  it("exposure as a fraction below one second", () => {
    expect(exposureText(1 / 250, "en")).toBe("1/250");
    expect(exposureText(2.5, "de")).toBe("2,5");
  });

  it("date taken in the language, as the camera wrote it", () => {
    expect(formatTaken("2023-05-12T14:30:00", "de")).toMatch(/12\.05\.2023.*14:30/);
    expect(formatTaken("2023-05-12T14:30:00", "ru")).toMatch(/12 мая 2023.*14:30/);
    expect(formatTaken("2023-05-12T14:30:00", "en")).toMatch(/May 12, 2023.*2:30/);
    expect(formatTaken("garbage", "en")).toBe("garbage");
  });

  it("numbers by language", () => {
    expect(numberText(5.6, "de", 1)).toBe("5,6");
    expect(numberText(6000, "en")).toBe("6,000");
  });
});
