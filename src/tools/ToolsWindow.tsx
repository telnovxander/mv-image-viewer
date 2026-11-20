import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen, History } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { DatesSection } from "./DatesSection";
import { DuplicatesSection } from "./DuplicatesSection";
import { applyBatch, cancelJob, indexFolder, inTauri, isBackendError, undoPlan, vollmondOperations, type IndexProgress, type IndexSummary, type Operation } from "../lib/backend";
import { formatDateTime } from "../lib/format";

let nextJob = 1;

/**
 * Vollmond's "Tools" (IV-13), loaded only when opened: the archive folder, its background
 * indexing with progress and stop, and the journal of operations. The tools themselves (dates,
 * duplicates, batch) come with IV-14…IV-19 and use the same index, preview and journal.
 */
interface ToolsWindowProps {
  protectedFolders: string[];
  onProtectedChange: (folders: string[]) => void;
}

export default function ToolsWindow({ protectedFolders, onProtectedChange }: ToolsWindowProps) {
  const { t, i18n } = useTranslation();
  const language = i18n.resolvedLanguage ?? "en";
  const [folder, setFolder] = useState<string | null>(null);
  const [recursive, setRecursive] = useState(true);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [summary, setSummary] = useState<IndexSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [operations, setOperations] = useState<Operation[]>([]);
  const job = useRef<number | null>(null);
  const [undoing, setUndoing] = useState<number | null>(null);

  const refreshJournal = () => void vollmondOperations().then(setOperations).catch(() => setOperations([]));

  /** Undo an operation: its changes reversed, through the same safe path, as a new journal entry. */
  const undo = async (op: number) => {
    setUndoing(op);
    setError(null);
    try {
      const plan = await undoPlan(op);
      const report = await applyBatch(plan, 0, plan.changes.length, op);
      if (report.error) setError(t(`error.${report.error.code}`, { path: report.error.path }));
    } catch (e) {
      setError(isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : String(e));
    }
    setUndoing(null);
    refreshJournal();
  };

  useEffect(() => {
    refreshJournal();
    return () => {
      if (job.current !== null) void cancelJob(job.current);
    };
  }, []);

  const chooseFolder = async () => {
    if (!inTauri) return;
    const chosen = await openDialog({ directory: true, multiple: false });
    if (typeof chosen === "string") setFolder(chosen);
  };

  const start = () => {
    if (!folder || !inTauri) return;
    const id = nextJob++;
    job.current = id;
    setSummary(null);
    setError(null);
    setProgress({ phase: "files", seen: 0 });
    void indexFolder(id, folder, recursive, (event) => {
      if (job.current !== id) return;
      if (event.event === "progress") setProgress(event.progress);
      else {
        if (event.event === "done") setSummary(event.summary);
        else setError(t(`error.${event.error.code}`, { path: event.error.path }));
        setProgress(null);
        job.current = null;
      }
    });
  };

  const stop = () => {
    if (job.current !== null) void cancelJob(job.current);
  };

  const running = progress !== null;
  const percent = progress && progress.phase !== "files" && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : null;

  return (
    <div className="tools">
      <section className="tools__section" aria-labelledby="tools-archive">
        <h3 id="tools-archive" className="tools__heading">
          {t("tools.archive")}
        </h3>
        <div className="tools__row">
          <button type="button" className="button" onClick={() => void chooseFolder()} disabled={running}>
            <FolderOpen size={16} aria-hidden="true" />
            {t("tools.choose_folder")}
          </button>
          <span className="tools__folder mono" title={folder ?? ""}>
            {folder ?? t("tools.no_folder")}
          </span>
        </div>
        <label className="field field--row">
          <input type="checkbox" checked={recursive} disabled={running} onChange={(event) => setRecursive(event.target.checked)} />
          <span>{t("tools.subfolders")}</span>
        </label>
        <div className="tools__row">
          <button type="button" className="button button--primary" disabled={!folder || running} onClick={start}>
            {t("tools.index")}
          </button>
          {running && (
            <button type="button" className="button" onClick={stop}>
              {t("tools.stop")}
            </button>
          )}
        </div>
        {running && (
          <div className="tools__progress" role="status">
            <progress max={100} value={percent ?? undefined} />
            <span>
              {progress.phase === "files"
                ? t("tools.progress_files", { count: progress.seen })
                : t(progress.phase === "hashes" ? "tools.progress_hashes" : "tools.progress_thumbnails", { done: progress.done, total: progress.total })}
            </span>
          </div>
        )}
        {summary && (
          <p className="dialog__text" role="status">
            {t(summary.cancelled ? "tools.summary_stopped" : "tools.summary", {
              count: summary.images,
              added: summary.added,
              changed: summary.changed,
              removed: summary.removed,
            })}
          </p>
        )}
        {error && (
          <p className="dialog__text tools__error" role="alert">
            {error}
          </p>
        )}
      </section>
      {folder && summary && <DatesSection folder={folder} recursive={recursive} onApplied={refreshJournal} />}
      {folder && summary && (
        <DuplicatesSection folder={folder} recursive={recursive} protectedFolders={protectedFolders} onProtectedChange={onProtectedChange} onApplied={refreshJournal} />
      )}
      <section className="tools__section" aria-labelledby="tools-journal">
        <h3 id="tools-journal" className="tools__heading">
          <History size={16} aria-hidden="true" />
          {t("tools.journal")}
        </h3>
        {operations.length === 0 ? (
          <p className="dialog__text dialog__muted">{t("tools.journal_empty")}</p>
        ) : (
          <ul className="tools__journal">
            {operations.map((op) => (
              <li key={op.op}>
                <span className="mono">{formatDateTime(op.time, language)}</span> · {t(`tools.tool.${op.tool}`, { defaultValue: op.tool })} ·{" "}
                {t("tools.files", { count: op.applied.length })}
                {op.interrupted && ` · ${t("tools.interrupted")}`}
                {op.undone && ` · ${t("tools.undone")}`}
                {op.undoes !== null && ` · ${t("tools.is_undo")}`}
                {!op.undone && op.undoes === null && op.applied.length > 0 && (
                  <button type="button" className="link-button tools__undo" disabled={undoing !== null} onClick={() => void undo(op.op)}>
                    {t("tools.undo")}
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
