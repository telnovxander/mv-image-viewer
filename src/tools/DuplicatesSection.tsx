import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Copy, Lock } from "lucide-react";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { applyBatch, cancelJob, duplicatesPreview, findDuplicates, isBackendError, thumbUrl, type ApplyReport, type DuplicateGroup, type Plan, type PlanSummary } from "../lib/backend";
import { defaultChoice, isProtected, toggleChoice } from "../lib/duplicates";
import { formatSize } from "../lib/format";
import { tip } from "./TipLayer";
import { stampText } from "../lib/dates";

/** Groups shown at once; more with "Show more" (a page stays light with thousands of groups). */
const PAGE = 100;

let nextJob = 1000;

interface DuplicatesSectionProps {
  folder: string;
  recursive: boolean;
  protectedFolders: string[];
  onProtectedChange: (folders: string[]) => void;
  onApplied: () => void;
}

/**
 * IV-17: exact duplicates of the indexed folder side by side, the original kept automatically
 * (changeable), protected folders never touched; the rest to the Recycle Bin after a preview, in
 * confirmed batches, with undo from the journal.
 */
export function DuplicatesSection({ folder, recursive, protectedFolders, onProtectedChange, onApplied }: DuplicatesSectionProps) {
  const { t, i18n } = useTranslation();
  const language = i18n.resolvedLanguage ?? "en";
  const [groups, setGroups] = useState<DuplicateGroup[] | null>(null);
  const [choice, setChoice] = useState<string[][]>([]);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [shown, setShown] = useState(PAGE);
  const [preview, setPreview] = useState<{ plan: Plan; summary: PlanSummary } | null>(null);
  const [nextBatch, setNextBatch] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ApplyReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const job = useRef<number | null>(null);

  const errorText = (e: unknown) => (isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : String(e));

  const search = () => {
    const id = nextJob++;
    job.current = id;
    setGroups(null);
    setPreview(null);
    setReport(null);
    setError(null);
    setProgress({ done: 0, total: 0 });
    void findDuplicates(id, folder, recursive, (event) => {
      if (job.current !== id) return;
      if (event.event === "progress") setProgress(event.progress);
      else {
        if (event.event === "done") {
          setGroups(event.groups);
          setChoice(defaultChoice(event.groups, protectedFolders));
          setShown(PAGE);
        } else setError(errorText(event.error));
        setProgress(null);
        job.current = null;
      }
    });
  };

  const makePreview = async () => {
    if (!groups) return;
    setError(null);
    setConfirming(false);
    setNextBatch(0);
    try {
      setPreview(await duplicatesPreview(groups, choice, protectedFolders));
    } catch (e) {
      setError(errorText(e));
    }
  };

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

  const addProtected = async () => {
    const chosen = await openDialog({ directory: true, multiple: false });
    if (typeof chosen === "string" && !protectedFolders.includes(chosen)) {
      const next = [...protectedFolders, chosen];
      onProtectedChange(next);
      // Files in the new protected folder leave the current choice at once.
      setChoice((current) => current.map((paths) => paths.filter((p) => !isProtected(p, next))));
      setPreview(null);
    }
  };

  const batches = preview?.summary.batches ?? [];
  const batch = batches[nextBatch];
  const chosenCount = choice.reduce((n, c) => n + c.length, 0);

  return (
    <section className="tools__section" aria-labelledby="tools-duplicates">
      <h3 id="tools-duplicates" className="tools__heading">
        <Copy size={16} aria-hidden="true" />
        {t("duplicates.title")}
      </h3>
      <p className="dialog__text tools__why">{t("duplicates.why")}</p>
      <div className="tools__row">
        <button type="button" className="button" disabled={progress !== null} onClick={search} {...tip(t("tips.dup_find"))}>
          {t("duplicates.find")}
        </button>
        {progress && (
          <>
            <progress max={Math.max(1, progress.total)} value={progress.done} />
            <button type="button" className="button" onClick={() => job.current !== null && void cancelJob(job.current)} {...tip(t("tips.stop"))}>
              {t("tools.stop")}
            </button>
          </>
        )}
      </div>
      <div className="tools__row tools__protected">
        <Lock size={14} aria-hidden="true" />
        <span>{protectedFolders.length === 0 ? t("duplicates.no_protected") : t("duplicates.protected")}</span>
        {protectedFolders.map((f) => (
          <span key={f} className="tools__chip mono">
            {f}
            <button type="button" className="icon-button" aria-label={t("duplicates.unprotect", { path: f })} {...tip(t("tips.unprotect"))} onClick={() => onProtectedChange(protectedFolders.filter((x) => x !== f))}>
              ×
            </button>
          </span>
        ))}
        <button type="button" className="link-button" onClick={() => void addProtected()} {...tip(t("tips.protect"))}>
          {t("duplicates.add_protected")}
        </button>
      </div>
      {groups && (
        <p className="dialog__text dialog__muted">
          {t("duplicates.found", { count: groups.length })} {t("duplicates.chosen", { count: chosenCount })}
        </p>
      )}
      {groups?.slice(0, shown).map((group, gi) => (
        <div key={group.files[0].path} className="dup__group">
          {group.files.map((f) => {
            const locked = isProtected(f.path, protectedFolders);
            const remove = choice[gi]?.includes(f.path) ?? false;
            return (
              <label key={f.path} className={`dup__file${remove ? " dup__file--remove" : ""}`} title={f.path}>
                <img src={thumbUrl(f.path)} alt="" loading="lazy" />
                <span className="dup__info">
                  <span className="mono">{f.path}</span>
                  <span className="dialog__muted">
                    {stampText(f.taken, language)} · {formatSize(f.size, language, t)}
                  </span>
                </span>
                {locked ? (
                  <Lock size={14} aria-label={t("duplicates.locked")} />
                ) : (
                  <span className="dup__toggle" {...tip(t("tips.dup_choose"))}>
                    <input
                      type="checkbox"
                      checked={remove}
                      onChange={() => {
                        setChoice((current) => toggleChoice(current, groups, gi, f.path));
                        setPreview(null);
                      }}
                    />
                    {t(remove ? "duplicates.to_bin" : "duplicates.keep")}
                  </span>
                )}
              </label>
            );
          })}
        </div>
      ))}
      {groups && groups.length > shown && (
        <button type="button" className="link-button" onClick={() => setShown((n) => n + PAGE)} {...tip(t("tips.more"))}>
          {t("duplicates.more", { count: groups.length - shown })}
        </button>
      )}
      {groups && chosenCount > 0 && (
        <div className="tools__row">
          <button type="button" className="button" disabled={busy} onClick={() => void makePreview()} {...tip(t("tips.dup_preview"))}>
            {t("duplicates.preview")}
          </button>
        </div>
      )}
      {preview && (
        <div className="tools__preview" role="status">
          <p className="dialog__text">{t("duplicates.summary", { count: preview.summary.files })}</p>
          {batches.length > 1 && <p className="dialog__text dialog__muted">{t("dates.batches", { count: batches.length })}</p>}
          {batch && !confirming && (
            <button type="button" className="button button--primary" disabled={busy} onClick={() => setConfirming(true)} {...tip(t("tips.apply_bin"))}>
              {t("duplicates.apply_batch", { n: nextBatch + 1, of: batches.length, count: batch[1] - batch[0] })}
            </button>
          )}
          {batch && confirming && (
            <div className="tools__row">
              <span>{t("duplicates.confirm", { count: batch[1] - batch[0] })}</span>
              <button type="button" className="button button--primary" disabled={busy} onClick={() => void applyNext()} {...tip(t("tips.confirm_bin"))}>
                {t("duplicates.confirm_yes")}
              </button>
              <button type="button" className="button" onClick={() => setConfirming(false)} {...tip(t("tips.cancel"))}>
                {t("common.cancel")}
              </button>
            </div>
          )}
          {!batch && report && <p className="dialog__text">{t("duplicates.all_done")}</p>}
        </div>
      )}
      {report && !report.error && <p className="dialog__text dialog__muted">{t("duplicates.applied", { count: report.applied })}</p>}
      {error && (
        <p className="dialog__text tools__error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
