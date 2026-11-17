import { MapPin } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ExifInfo, ImageFile } from "../lib/backend";
import { exposureText, formatSize, formatTaken, numberText } from "../lib/format";
import type { Size } from "../lib/zoom";

interface InfoPanelProps {
  file: ImageFile;
  size: Size | null;
  exif: ExifInfo | null;
  onOpenMap: (latitude: number, longitude: number) => void;
}

/** I: the information panel — file, size, resolution and the EXIF fields (IV-04). */
export function InfoPanel({ file, size, exif, onOpenMap }: InfoPanelProps) {
  const { t, i18n } = useTranslation();
  const language = i18n.language;
  const rows: [string, string | null][] = [
    ["name", file.name],
    ["size", formatSize(file.size, language, t)],
    ["resolution", size ? t("info.pixels", { width: numberText(size.width, language), height: numberText(size.height, language) }) : null],
    ["taken", exif?.taken ? formatTaken(exif.taken, language) : null],
    ["camera", exif?.camera ?? null],
    ["lens", exif?.lens ?? null],
    ["exposure", exif?.exposure ? t("info.seconds", { value: exposureText(exif.exposure, language) }) : null],
    ["aperture", exif?.f_number ? `f/${numberText(exif.f_number, language, 1)}` : null],
    ["iso", exif?.iso ? numberText(exif.iso, language) : null],
    ["focal_length", exif?.focal_length ? t("info.millimetres", { value: numberText(exif.focal_length, language, 1) }) : null],
  ];
  const gps = exif?.gps ?? null;
  return (
    <aside className="info-panel" aria-label={t("info.title")}>
      <h2 className="info-panel__title">{t("info.title")}</h2>
      <dl className="info-panel__list">
        {rows
          .filter(([, value]) => value !== null && value !== "")
          .map(([key, value]) => (
            <div key={key} className="info-panel__row">
              <dt>{t(`info.${key}`)}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        {gps && (
          <div className="info-panel__row">
            <dt>{t("info.gps")}</dt>
            <dd>
              {numberText(gps[0], language, 5)}, {numberText(gps[1], language, 5)}
              <button type="button" className="link-button" onClick={() => onOpenMap(gps[0], gps[1])}>
                <MapPin size={14} aria-hidden="true" />
                {t("info.open_map")}
              </button>
            </dd>
          </div>
        )}
      </dl>
      {!exif && <p className="info-panel__muted">{t("info.no_exif")}</p>}
    </aside>
  );
}
