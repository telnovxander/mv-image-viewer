import { useState } from "react";
import { useTranslation } from "react-i18next";
import { applyBatch, isBackendError, manualDatesPreview, type ManualDates, type Plan, type PlanSummary } from "../lib/backend";
import { stampText } from "../lib/dates";
import { Dialog } from "./Dialog";

interface DateEditDialogProps {
  path: string;
  name: string;
  /** The current date taken, `YYYY-MM-DDTHH:MM:SS`, or null. */
  taken: string | null;
  /** Applied: the journal operation, for undo. */
  onDone: (op: number) => void;
  onClose: () => void;
}

/** `datetime-local` value (minutes or seconds) → a stamp with seconds. */
export function inputToStamp(value: string): string | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?$/.exec(value);
  return m ? `${m[1]}-${m[2]}-${m[3]}T${m[4]}:${m[5]}:${m[6] ?? "00"}` : null;
}

/**
 * D in Sichel (IV-15): the date of the current photo by hand — exact, or approximate (a year, or a
 * month of a year: the middle is written and the journal says so). Preview "before → after" first,
 * then the same safe write and journal as the Vollmond tools; undo from the notice.
 */
export function DateEditDialog({ path, name, taken, onDone, onClose }: DateEditDialogProps) {
  const { t, i18n } = useTranslation();
  const language = i18n.resolvedLanguage ?? "en";
  const [exact, setExact] = useState(true);
  const [value, setValue] = useState((taken ?? "").slice(0, 19));
  const [year, setYear] = useState(taken ? Number(taken.slice(0, 4)) : 2000);
  const [month, setMonth] = useState(0);
  const [withSuspicious, setWithSuspicious] = useState(false);
  const [preview, setPreview] = useState<{ plan: Plan; summary: PlanSummary } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const how = (): ManualDates | null => {
    if (!exact) return { mode: "approximate", year, ...(month > 0 ? { month } : {}) };
    const stamp = inputToStamp(value);
    return stamp ? { mode: "same", date: stamp } : null;
  };

  const show = async (suspicious = withSuspicious) => {
    const chosen = how();
    setError(null);
    if (!chosen) return setPreview(null);
    try {
      setPreview(await manualDatesPreview([path], chosen, suspicious));
    } catch (e) {
      setError(isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : String(e));
    }
  };

  const apply = async () => {
    if (!preview) return;
    try {
      const report = await applyBatch(preview.plan, 0, preview.plan.changes.length, null);
      if (report.error) setError(t(`error.${report.error.code}`, { path: report.error.path }));
      else onDone(report.op);
    } catch (e) {
      setError(isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : String(e));
    }
  };

  const overwrites = preview?.plan.changes.some((c) => c.field === "taken" && c.before !== null) ?? false;

  return (
    <Dialog title={t("date_edit.title")} onClose={onClose}>
      <p className="dialog__text">{name}</p>
      <div className="segmented" role="radiogroup" aria-label={t("date_edit.kind")}>
        {[true, false].map((kind) => (
          <label key={String(kind)} className={`segmented__item${exact === kind ? " segmented__item--on" : ""}`}>
            <input
              type="radio"
              name="date-kind"
              checked={exact === kind}
              onChange={() => {
                setExact(kind);
                setPreview(null);
              }}
            />
            {t(kind ? "date_edit.exact" : "date_edit.approximate")}
          </label>
        ))}
      </div>
      {exact ? (
        <label className="field field--row">
          <span>{t("date_edit.date")}</span>
          <input
            type="datetime-local"
            step={1}
            autoFocus
            value={value}
            onChange={(event) => {
              setValue(event.target.value);
              setPreview(null);
            }}
          />
        </label>
      ) : (
        <div className="tools__row">
          <label className="field field--row tools__inline">
            <span>{t("date_edit.year")}</span>
            <input className="number-input" type="number" min={1826} max={2200} value={year} onChange={(event) => setYear(Number(event.target.value))} />
          </label>
          <label className="field field--row tools__inline">
            <span>{t("date_edit.month")}</span>
            <select className="select" value={month} onChange={(event) => setMonth(Number(event.target.value))}>
              <option value={0}>{t("date_edit.whole_year")}</option>
              {Array.from({ length: 12 }, (_, i) => (
                <option key={i + 1} value={i + 1}>
                  {new Intl.DateTimeFormat(language, { month: "long" }).format(new Date(2000, i, 1))}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
      <div className="tools__row">
        <button type="button" className="button" onClick={() => void show()}>
          {t("date_edit.preview")}
        </button>
        <label className="field field--row tools__inline">
          <input
            type="checkbox"
            checked={withSuspicious}
            onChange={(event) => {
              setWithSuspicious(event.target.checked);
              if (preview) void show(event.target.checked);
            }}
          />
          <span>{t("dates.with_suspicious")}</span>
        </label>
      </div>
      {preview && (
        <table className="plan">
          <thead>
            <tr>
              <th>{t("date_edit.what")}</th>
              <th>{t("write_rotation.from")}</th>
              <th>{t("write_rotation.to")}</th>
            </tr>
          </thead>
          <tbody>
            {preview.plan.changes.map((c) => (
              <tr key={c.field}>
                <td>{t(`date_edit.field_${c.field}`)}</td>
                <td>{stampText(c.before, language)}</td>
                <td>{stampText(c.after, language)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {preview && preview.plan.changes.length === 0 && <p className="dialog__text dialog__muted">{t("date_edit.nothing")}</p>}
      {overwrites && <p className="dialog__text tools__warning">⚠ {t("date_edit.overwrites")}</p>}
      {error && (
        <p className="dialog__text tools__error" role="alert">
          {error}
        </p>
      )}
      <div className="dialog__actions">
        <button type="button" className="button" onClick={onClose}>
          {t("common.cancel")}
        </button>
        <button type="button" className="button button--primary" disabled={!preview || preview.plan.changes.length === 0} onClick={() => void apply()}>
          {t("date_edit.apply")}
        </button>
      </div>
    </Dialog>
  );
}
