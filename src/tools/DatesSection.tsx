import { CalendarClock } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { inputToStamp } from "../components/DateEditDialog";
import { applyBatch, datesPreview, isBackendError, manualDatesPreview, datesRows, type ManualDates, type ApplyReport, type DateRow, type Plan, type PlanSummary } from "../lib/backend";
import { filterRows, stampText, visibleRows, type DateFilter } from "../lib/dates";

const ROW = 22;
const VIEW = 264;

interface DatesSectionProps {
  folder: string;
  recursive: boolean;
  /** A batch was applied: the journal shows it. */
  onApplied: () => void;
}

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/**
 * IV-14: dates of the indexed folder — EXIF, file name, file time — with mismatches lit, three
 * filters, and fixing through the shared preview and journal (IV-13): totals first, batches of at
 * most 500 (a trial of 10 for 100+), each confirmed with its number of files.
 */
export function DatesSection({ folder, recursive, onApplied }: DatesSectionProps) {
  const { t, i18n } = useTranslation();
  const language = i18n.resolvedLanguage ?? "en";
  const [rows, setRows] = useState<DateRow[] | null>(null);
  const [filter, setFilter] = useState<DateFilter>("mismatches");
  const [top, setTop] = useState(0);
  const [preview, setPreview] = useState<{ plan: Plan; summary: PlanSummary } | null>(null);
  const [withSuspicious, setWithSuspicious] = useState(false);
  const [nextBatch, setNextBatch] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ApplyReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** IV-15: the rows chosen for a date by hand, in table order. */
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set());
  const [manualMode, setManualMode] = useState<"same" | "step" | "shift" | "approximate">("same");
  const [manualDate, setManualDate] = useState("");
  const [stepMinutes, setStepMinutes] = useState(1);
  const [shiftHours, setShiftHours] = useState(0);
  const [shiftMinutes, setShiftMinutes] = useState(0);
  const [approxYear, setApproxYear] = useState(2005);
  const [approxPart, setApproxPart] = useState(0);

  const errorText = (e: unknown) =>
    isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : String(e);

  const load = async () => {
    setError(null);
    setPreview(null);
    setReport(null);
    try {
      setRows(await datesRows(folder, recursive));
    } catch (e) {
      setError(errorText(e));
    }
  };

  const makePreview = async (suspicious = withSuspicious) => {
    setError(null);
    setReport(null);
    setConfirming(false);
    setNextBatch(0);
    try {
      setPreview(await datesPreview(folder, recursive, suspicious));
    } catch (e) {
      setError(errorText(e));
    }
  };

  const manualHow = (): ManualDates | null => {
    if (manualMode === "shift") return { mode: "shift", seconds: shiftHours * 3600 + shiftMinutes * 60 };
    if (manualMode === "approximate") {
      // 1–12: a month; 13–16: spring, summer, autumn, winter; 0: the whole year.
      if (approxPart >= 13) return { mode: "approximate", year: approxYear, season: approxPart - 12 };
      return { mode: "approximate", year: approxYear, ...(approxPart > 0 ? { month: approxPart } : {}) };
    }
    const stamp = inputToStamp(manualDate);
    if (!stamp) return null;
    return manualMode === "same" ? { mode: "same", date: stamp } : { mode: "step", start: stamp, step_seconds: stepMinutes * 60 };
  };

  const manualPreview = async (suspicious = withSuspicious) => {
    const how = manualHow();
    if (!how || !rows) return;
    setError(null);
    setReport(null);
    setConfirming(false);
    setNextBatch(0);
    const paths = rows.filter((r) => chosen.has(r.path)).map((r) => r.path);
    try {
      setPreview(await manualDatesPreview(paths, how, suspicious));
    } catch (e) {
      setError(errorText(e));
    }
  };

  const toggle = (path: string) =>
    setChosen((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  const applyNext = async () => {
    if (!preview) return;
    const [start, end] = preview.summary.batches[nextBatch];
    setBusy(true);
    setConfirming(false);
    try {
      const done = await applyBatch(preview.plan, start, end, null);
      setReport(done);
      onApplied();
      if (done.error) setError(errorText(done.error));
      else setNextBatch((n) => n + 1);
    } catch (e) {
      setError(errorText(e));
    }
    setBusy(false);
  };

  const shown = rows ? filterRows(rows, filter) : [];
  const [first, last] = visibleRows(top, VIEW, shown.length, ROW);
  const batches = preview?.summary.batches ?? [];
  const batch = batches[nextBatch];

  return (
    <section className="tools__section" aria-labelledby="tools-dates">
      <h3 id="tools-dates" className="tools__heading">
        <CalendarClock size={16} aria-hidden="true" />
        {t("dates.title")}
      </h3>
      <div className="tools__row">
        <button type="button" className="button" onClick={() => void load()}>
          {t(rows ? "dates.reload" : "dates.show")}
        </button>
        {rows && (
          <div className="segmented" role="radiogroup" aria-label={t("dates.filter")}>
            {(["all", "mismatches", "no_date_taken"] as const).map((f) => (
              <label key={f} className={`segmented__item${filter === f ? " segmented__item--on" : ""}`}>
                <input type="radio" name="dates-filter" checked={filter === f} onChange={() => setFilter(f)} />
                {t(`dates.filter_${f}`)}
              </label>
            ))}
          </div>
        )}
      </div>
      {rows && (
        <>
          <p className="dialog__text dialog__muted">{t("dates.count", { count: shown.length, total: rows.length })}</p>
          <div className="dates__table" role="table" aria-label={t("dates.title")}>
            <div className="dates__row dates__row--head" role="row">
              <span role="columnheader">
                <input
                  type="checkbox"
                  aria-label={t("dates.choose_shown")}
                  checked={shown.length > 0 && shown.every((r) => chosen.has(r.path))}
                  onChange={(event) => setChosen(event.target.checked ? new Set(shown.map((r) => r.path)) : new Set())}
                />{" "}
                {t("dates.file")}
              </span>
              <span role="columnheader">{t("dates.taken")}</span>
              <span role="columnheader">{t("dates.from_name")}</span>
              <span role="columnheader">{t("dates.modified")}</span>
              <span role="columnheader">{t("dates.proposed")}</span>
            </div>
            <div className="dates__body" style={{ height: VIEW }} onScroll={(event) => setTop(event.currentTarget.scrollTop)}>
              <div style={{ height: shown.length * ROW, position: "relative" }}>
                {shown.slice(first, last).map((r, i) => (
                  <div
                    key={r.path}
                    role="row"
                    className={`dates__row${r.mismatch ? " dates__row--mismatch" : ""}${r.suspicious ? " dates__row--suspicious" : ""}`}
                    style={{ transform: `translateY(${(first + i) * ROW}px)` }}
                    title={r.path}
                  >
                    <span>
                      <input type="checkbox" aria-label={baseName(r.path)} checked={chosen.has(r.path)} onChange={() => toggle(r.path)} /> {baseName(r.path)}
                    </span>
                    <span className="mono">{stampText(r.taken, language)}</span>
                    <span className="mono">{stampText(r.from_name?.stamp ?? null, language)}</span>
                    <span className="mono">{stampText(r.modified, language)}</span>
                    <span className="mono">
                      {stampText(r.proposed, language)}
                      {r.suspicious && ` ⚠ ${t("dates.suspicious")}`}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </div>
          <div className="tools__row">
            <button type="button" className="button" disabled={busy} onClick={() => void makePreview()}>
              {t("dates.preview")}
            </button>
            <label className="field field--row tools__inline">
              <input
                type="checkbox"
                checked={withSuspicious}
                onChange={(event) => {
                  setWithSuspicious(event.target.checked);
                  if (preview) void makePreview(event.target.checked);
                }}
              />
              <span>{t("dates.with_suspicious")}</span>
            </label>
          </div>
        </>
      )}
      {rows && chosen.size > 0 && (
        <div className="tools__manual">
          <p className="dialog__text">{t("dates.manual_title", { count: chosen.size })}</p>
          <div className="tools__row">
            <select className="select" aria-label={t("dates.manual_how")} value={manualMode} onChange={(event) => setManualMode(event.target.value as typeof manualMode)}>
              {(["same", "step", "shift", "approximate"] as const).map((m) => (
                <option key={m} value={m}>
                  {t(`dates.manual_${m}`)}
                </option>
              ))}
            </select>
            {(manualMode === "same" || manualMode === "step") && (
              <input type="datetime-local" step={1} aria-label={t("date_edit.date")} value={manualDate} onChange={(event) => setManualDate(event.target.value)} />
            )}
            {manualMode === "step" && (
              <label className="field field--row tools__inline">
                <span>{t("dates.manual_step_minutes")}</span>
                <input className="number-input" type="number" min={1} value={stepMinutes} onChange={(event) => setStepMinutes(Math.max(1, Number(event.target.value)))} />
              </label>
            )}
            {manualMode === "shift" && (
              <>
                <label className="field field--row tools__inline">
                  <span>{t("dates.manual_hours")}</span>
                  <input className="number-input" type="number" value={shiftHours} onChange={(event) => setShiftHours(Number(event.target.value))} />
                </label>
                <label className="field field--row tools__inline">
                  <span>{t("dates.manual_minutes")}</span>
                  <input className="number-input" type="number" value={shiftMinutes} onChange={(event) => setShiftMinutes(Number(event.target.value))} />
                </label>
              </>
            )}
            {manualMode === "approximate" && (
              <>
                <input className="number-input" type="number" aria-label={t("date_edit.year")} min={1826} max={2200} value={approxYear} onChange={(event) => setApproxYear(Number(event.target.value))} />
                <select className="select" aria-label={t("date_edit.month")} value={approxPart} onChange={(event) => setApproxPart(Number(event.target.value))}>
                  <option value={0}>{t("date_edit.whole_year")}</option>
                  {[1, 2, 3, 4].map((season) => (
                    <option key={`s${season}`} value={12 + season}>
                      {t(`dates.season_${season}`)}
                    </option>
                  ))}
                  {Array.from({ length: 12 }, (_, i) => (
                    <option key={i + 1} value={i + 1}>
                      {new Intl.DateTimeFormat(language, { month: "long" }).format(new Date(2000, i, 1))}
                    </option>
                  ))}
                </select>
              </>
            )}
            <button type="button" className="button" disabled={busy || manualHow() === null} onClick={() => void manualPreview()}>
              {t("dates.manual_preview")}
            </button>
          </div>
        </div>
      )}
      {preview && (
        <div className="tools__preview" role="status">
          <p className="dialog__text">
            {t("dates.summary", { count: preview.summary.files, changes: preview.summary.changes })}
            {preview.summary.suspicious > 0 && ` ${t("dates.summary_suspicious", { count: preview.summary.suspicious })}`}
          </p>
          {preview.plan.changes.some((c) => c.field === "taken" && c.before !== null) && (
            <p className="dialog__text tools__warning">⚠ {t("dates.overwrites")}</p>
          )}
          {preview.summary.warnings.map(([path, kind]) => (
            <p key={path} className="dialog__text tools__warning">
              ⚠ {t(`dates.warning_${kind}`, { path })}
            </p>
          ))}
          {batches.length > 1 && <p className="dialog__text dialog__muted">{t("dates.batches", { count: batches.length })}</p>}
          {batch && !confirming && (
            <button type="button" className="button button--primary" disabled={busy || preview.summary.changes === 0} onClick={() => setConfirming(true)}>
              {t("dates.apply_batch", { n: nextBatch + 1, of: batches.length, count: batch[1] - batch[0] })}
            </button>
          )}
          {batch && confirming && (
            <div className="tools__row">
              <span>{t("dates.confirm", { count: batch[1] - batch[0] })}</span>
              <button type="button" className="button button--primary" disabled={busy} onClick={() => void applyNext()}>
                {t("dates.confirm_yes")}
              </button>
              <button type="button" className="button" onClick={() => setConfirming(false)}>
                {t("common.cancel")}
              </button>
            </div>
          )}
          {!batch && report && <p className="dialog__text">{t("dates.all_done")}</p>}
        </div>
      )}
      {report && !report.error && <p className="dialog__text dialog__muted">{t("dates.applied", { count: report.applied })}</p>}
      {error && (
        <p className="dialog__text tools__error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
