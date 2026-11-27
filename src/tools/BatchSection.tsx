import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Layers } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { applyBatch, batchPreview, isBackendError, type ApplyReport, type Plan, type Planned, type PlanSummary } from "../lib/backend";
import { tip } from "./TipLayer";
import { BUILT_IN, EMPTY_CHAIN, stepsOf, type Chain, type Preset } from "../lib/batch";

interface BatchSectionProps {
  folder: string;
  recursive: boolean;
  presets: Preset[];
  onPresetsChange: (presets: Preset[]) => void;
  onApplied: () => void;
}

/**
 * IV-19: a chain of steps applied to copies in a new folder — the originals are only read. Presets
 * ("For a messenger", "For publishing" and the user's own), a preview of names and sizes, batches
 * confirmed one by one, undo from the journal (the copies go to the Recycle Bin).
 */
export function BatchSection({ folder, recursive, presets, onPresetsChange, onApplied }: BatchSectionProps) {
  const { t } = useTranslation();
  const [chain, setChain] = useState<Chain>(EMPTY_CHAIN);
  const [outDir, setOutDir] = useState<string | null>(null);
  const [preview, setPreview] = useState<{ plan: Plan; summary: PlanSummary; planned: Planned[] } | null>(null);
  const [nextBatch, setNextBatch] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ApplyReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [presetName, setPresetName] = useState("");

  const errorText = (e: unknown) => (isBackendError(e) ? t(`error.${e.code}`, { path: e.path }) : String(e));
  const change = (part: Partial<Chain>) => {
    setChain((c) => ({ ...c, ...part }));
    setPreview(null);
  };

  const chooseOut = async () => {
    const chosen = await openDialog({ directory: true, multiple: false });
    if (typeof chosen === "string") {
      setOutDir(chosen);
      setPreview(null);
    }
  };

  const makePreview = async () => {
    if (!outDir) return;
    setError(null);
    setReport(null);
    setConfirming(false);
    setNextBatch(0);
    try {
      setPreview(await batchPreview(folder, recursive, stepsOf(chain), outDir));
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

  const all = [...BUILT_IN.map((p) => ({ ...p, label: t(`batch.preset_${p.name}`), own: false })), ...presets.map((p) => ({ ...p, label: p.name, own: true }))];
  const batches = preview?.summary.batches ?? [];
  const batch = batches[nextBatch];

  return (
    <section className="tools__section" aria-labelledby="tools-batch">
      <h3 id="tools-batch" className="tools__heading">
        <Layers size={16} aria-hidden="true" />
        {t("batch.title")}
      </h3>
      <p className="dialog__text tools__why">{t("batch.why")}</p>
      <div className="tools__row tools__wrap">
        <span>{t("batch.presets")}</span>
        {all.map((p) => (
          <span key={`${p.own}-${p.name}`} className="tools__chip">
            <button type="button" className="link-button" onClick={() => change(p.chain)} {...tip(t("tips.preset"))}>
              {p.label}
            </button>
            {p.own && (
              <button type="button" className="icon-button" aria-label={t("batch.delete_preset", { name: p.name })} {...tip(t("tips.delete_preset"))} onClick={() => onPresetsChange(presets.filter((x) => x.name !== p.name))}>
                ×
              </button>
            )}
          </span>
        ))}
      </div>
      <div className="batch__steps">
        <label className="field field--row tools__inline" {...tip(t("tips.resize"))}>
          <input type="checkbox" checked={chain.resize !== null} onChange={(e) => change({ resize: e.target.checked ? 2048 : null })} />
          <span>{t("batch.resize")}</span>
          {chain.resize !== null && <input className="number-input" type="number" min={64} value={chain.resize} onChange={(e) => change({ resize: Math.max(64, Number(e.target.value)) })} />}
        </label>
        <label className="field field--row tools__inline" {...tip(t("tips.fit"))}>
          <input type="checkbox" checked={chain.fitMb !== null} onChange={(e) => change({ fitMb: e.target.checked ? 2 : null })} />
          <span>{t("batch.fit")}</span>
          {chain.fitMb !== null && <input className="number-input" type="number" min={0.1} step={0.1} value={chain.fitMb} onChange={(e) => change({ fitMb: Math.max(0.1, Number(e.target.value)) })} />}
        </label>
        <label className="field field--row tools__inline" {...tip(t("tips.convert"))}>
          <span>{t("batch.convert")}</span>
          <select className="select" value={chain.convert} onChange={(e) => change({ convert: e.target.value as Chain["convert"] })}>
            <option value="keep">{t("batch.convert_keep")}</option>
            <option value="jpeg">{t("batch.convert_jpeg")}</option>
            <option value="png">{t("batch.convert_png")}</option>
          </select>
        </label>
        <label className="field field--row tools__inline" {...tip(t("tips.metadata"))}>
          <span>{t("batch.metadata")}</span>
          <select className="select" value={chain.metadata} onChange={(e) => change({ metadata: e.target.value as Chain["metadata"] })}>
            <option value="keep">{t("batch.metadata_keep")}</option>
            <option value="strip_gps">{t("batch.metadata_gps")}</option>
            <option value="strip_all">{t("batch.metadata_all")}</option>
          </select>
        </label>
        <label className="field field--row tools__inline" {...tip(t("tips.rename"))}>
          <input type="checkbox" checked={chain.rename !== null} onChange={(e) => change({ rename: e.target.checked ? "{YYYY}-{MM}-{DD}_{hh}-{mm}-{ss}" : null })} />
          <span>{t("batch.rename")}</span>
          {chain.rename !== null && <input className="panel-input mono" value={chain.rename} spellCheck={false} onChange={(e) => change({ rename: e.target.value })} />}
        </label>
      </div>
      <div className="tools__row">
        <input className="mono" placeholder={t("batch.preset_name")} aria-label={t("batch.preset_name")} {...tip(t("tips.preset_name"))} value={presetName} onChange={(e) => setPresetName(e.target.value)} />
        <button
          type="button"
          className="button"
          {...tip(t("tips.save_preset"))}
          disabled={presetName.trim() === ""}
          onClick={() => {
            const name = presetName.trim();
            onPresetsChange([...presets.filter((p) => p.name !== name), { name, chain }]);
            setPresetName("");
          }}
        >
          {t("batch.save_preset")}
        </button>
      </div>
      <div className="tools__row">
        <button type="button" className="button" onClick={() => void chooseOut()} {...tip(t("tips.out_dir"))}>
          {t("batch.out_dir")}
        </button>
        <span className="tools__folder mono">{outDir ?? t("batch.no_out_dir")}</span>
      </div>
      <div className="tools__row">
        <button type="button" className="button" disabled={!outDir || busy} onClick={() => void makePreview()} {...tip(t("tips.batch_preview"))}>
          {t("batch.preview")}
        </button>
      </div>
      {preview && (
        <div className="tools__preview" role="status">
          <p className="dialog__text">{t("batch.summary", { count: preview.planned.length })}</p>
          <ul className="tools__journal">
            {preview.planned.slice(0, 200).map((p) => (
              <li key={p.source} className="mono">
                {p.source.split(/[\\/]/).pop()} → {p.target.split(/[\\/]/).pop()} · {p.width}×{p.height}
              </li>
            ))}
          </ul>
          {preview.planned.length > 200 && <p className="dialog__text dialog__muted">{t("batch.more", { count: preview.planned.length - 200 })}</p>}
          {preview.summary.warnings.map(([path, kind]) => (
            <p key={path} className="dialog__text tools__warning">
              ⚠ {t(`dates.warning_${kind}`, { path })}
            </p>
          ))}
          {batches.length > 1 && <p className="dialog__text dialog__muted">{t("dates.batches", { count: batches.length })}</p>}
          {batch && !confirming && (
            <button type="button" className="button button--primary" disabled={busy} onClick={() => setConfirming(true)} {...tip(t("tips.apply_copies"))}>
              {t("batch.apply_batch", { n: nextBatch + 1, of: batches.length, count: batch[1] - batch[0] })}
            </button>
          )}
          {batch && confirming && (
            <div className="tools__row">
              <span>{t("batch.confirm", { count: batch[1] - batch[0] })}</span>
              <button type="button" className="button button--primary" disabled={busy} onClick={() => void applyNext()} {...tip(t("tips.confirm_copies"))}>
                {t("batch.confirm_yes")}
              </button>
              <button type="button" className="button" onClick={() => setConfirming(false)} {...tip(t("tips.cancel"))}>
                {t("common.cancel")}
              </button>
            </div>
          )}
          {!batch && report && <p className="dialog__text">{t("batch.all_done")}</p>}
        </div>
      )}
      {error && (
        <p className="dialog__text tools__error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
