import { Images, Star } from "lucide-react";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { applyBatch, cancelJob, duplicatesPreview, findSimilar, isBackendError, thumbUrl, type ApplyReport, type Plan, type PlanSummary, type SimilarGroup } from "../lib/backend";
import { isProtected, toggleChoice } from "../lib/duplicates";
import { formatSize, numberText } from "../lib/format";
import { tip } from "./TipLayer";
import { CompareDialog } from "./CompareDialog";

const PAGE = 100;
let nextJob = 2000;

interface SimilarSectionProps {
  folder: string;
  recursive: boolean;
  protectedFolders: string[];
  onApplied: () => void;
}

/**
 * IV-18: similar photos (bursts, other sizes and qualities, edited copies) with a threshold. The
 * best frame is only suggested (★); nothing is chosen for deletion until the user ticks it (point
 * 18). Two ticked or chosen frames can be compared side by side with one zoom.
 */
export function SimilarSection({ folder, recursive, protectedFolders, onApplied }: SimilarSectionProps) {
  const { t, i18n } = useTranslation();
  const language = i18n.resolvedLanguage ?? "en";
  const [threshold, setThreshold] = useState(10);
  const [groups, setGroups] = useState<SimilarGroup[] | null>(null);
  const [choice, setChoice] = useState<string[][]>([]);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [shown, setShown] = useState(PAGE);
  const [compare, setCompare] = useState<string[]>([]);
  const [comparing, setComparing] = useState(false);
  const [preview, setPreview] = useState<{ plan: Plan; summary: PlanSummary } | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [nextBatch, setNextBatch] = useState(0);
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
    setCompare([]);
    setProgress({ done: 0, total: 0 });
    void findSimilar(id, folder, recursive, threshold, (event) => {
      if (job.current !== id) return;
      if (event.event === "progress") setProgress(event.progress);
      else {
        if (event.event === "done") {
          setGroups(event.groups);
          // Point 18: nothing is chosen automatically.
          setChoice(event.groups.map(() => []));
          setShown(PAGE);
        } else setError(errorText(event.error));
        setProgress(null);
        job.current = null;
      }
    });
  };

  const asGroups = () => (groups ?? []).map((g) => ({ files: g.files, keep: g.best }));

  const makePreview = async () => {
    setError(null);
    setConfirming(false);
    setNextBatch(0);
    try {
      setPreview(await duplicatesPreview(asGroups(), choice, protectedFolders));
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

  const toggleCompare = (path: string) =>
    setCompare((current) => (current.includes(path) ? current.filter((p) => p !== path) : [...current, path].slice(-2)));

  const batches = preview?.summary.batches ?? [];
  const batch = batches[nextBatch];
  const chosenCount = choice.reduce((n, c) => n + c.length, 0);

  return (
    <section className="tools__section" aria-labelledby="tools-similar">
      <h3 id="tools-similar" className="tools__heading">
        <Images size={16} aria-hidden="true" />
        {t("similar.title")}
      </h3>
      <p className="dialog__text tools__why">{t("similar.why")}</p>
      <div className="tools__row">
        <label className="field field--row tools__inline" {...tip(t("tips.similar_threshold"))}>
          <span>{t("similar.threshold")}</span>
          <input type="range" min={2} max={20} value={threshold} onChange={(event) => setThreshold(Number(event.target.value))} />
          <span className="mono">{threshold}</span>
        </label>
        <button type="button" className="button" disabled={progress !== null} onClick={search} {...tip(t("tips.similar_find"))}>
          {t("similar.find")}
        </button>
        {progress && (
          <>
            <progress max={Math.max(1, progress.total)} value={progress.done} />
            <button type="button" className="button" onClick={() => job.current !== null && void cancelJob(job.current)} {...tip(t("tips.stop"))}>
              {t("tools.stop")}
            </button>
          </>
        )}
        <button type="button" className="button" disabled={compare.length !== 2} onClick={() => setComparing(true)} {...tip(t("tips.compare"))}>
          {t("similar.compare", { count: compare.length })}
        </button>
      </div>
      {groups && <p className="dialog__text dialog__muted">{t("similar.found", { count: groups.length })}</p>}
      {groups?.slice(0, shown).map((group, gi) => (
        <div key={group.files[0].path} className="dup__group">
          {group.files.map((f, fi) => {
            const locked = isProtected(f.path, protectedFolders);
            const remove = choice[gi]?.includes(f.path) ?? false;
            const q = group.qualities[fi];
            return (
              <div key={f.path} className={`dup__file${remove ? " dup__file--remove" : ""}`} title={f.path}>
                <img src={thumbUrl(f.path)} alt="" loading="lazy" />
                <span className="dup__info">
                  <span className="mono">
                    {fi === group.best && <Star size={12} aria-label={t("similar.best")} className="similar__best" />} {f.path}
                  </span>
                  <span className="dialog__muted">
                    {formatSize(f.size, language, t)} · {t("similar.quality", { sharp: numberText(q.sharpness, language), exposure: numberText(q.exposure * 100, language) })}
                  </span>
                </span>
                <span className="dup__toggle">
                  <label {...tip(t("tips.to_compare"))}>
                    <input type="checkbox" checked={compare.includes(f.path)} onChange={() => toggleCompare(f.path)} /> {t("similar.to_compare")}
                  </label>
                  {!locked && (
                    <label {...tip(t("tips.dup_choose"))}>
                      <input
                        type="checkbox"
                        checked={remove}
                        onChange={() => {
                          setChoice((current) => toggleChoice(current, asGroups(), gi, f.path));
                          setPreview(null);
                        }}
                      />{" "}
                      {t("duplicates.to_bin")}
                    </label>
                  )}
                </span>
              </div>
            );
          })}
        </div>
      ))}
      {groups && groups.length > shown && (
        <button type="button" className="link-button" onClick={() => setShown((n) => n + PAGE)} {...tip(t("tips.more"))}>
          {t("duplicates.more", { count: groups.length - shown })}
        </button>
      )}
      {chosenCount > 0 && (
        <div className="tools__row">
          <button type="button" className="button" disabled={busy} onClick={() => void makePreview()} {...tip(t("tips.dup_preview"))}>
            {t("duplicates.preview")}
          </button>
        </div>
      )}
      {preview && (
        <div className="tools__preview" role="status">
          <p className="dialog__text">{t("duplicates.summary", { count: preview.summary.files })}</p>
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
      {error && (
        <p className="dialog__text tools__error" role="alert">
          {error}
        </p>
      )}
      {comparing && compare.length === 2 && <CompareDialog left={compare[0]} right={compare[1]} onClose={() => setComparing(false)} />}
    </section>
  );
}
