import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Icon } from "./Icon";
import { message } from "./RecentCatches";
import { preferredFont, useFontPreview } from "../lib/font-preview";
import {
  fileName,
  sourceDomain,
  type InstalledFont,
  type Snapshot,
  type UninstallPlan,
  type UninstallOutcome,
} from "../lib/native";
import type { Language } from "../lib/types";
import fr from "../locales/fr.json";
type Copy = typeof fr;

export function NativeFontDetail({
  family,
  fonts,
  t,
  language,
  onBack,
  onSnapshot,
  onNotice,
}: {
  family: string;
  fonts: InstalledFont[];
  t: Copy;
  language: Language;
  onBack: () => void;
  onSnapshot: (s: Snapshot) => void;
  onNotice: (text: string) => void;
}) {
  const [postscript, setPostscript] = useState(
    preferredFont(fonts)?.face.postscript ?? "",
  );
  const selected =
    fonts.find((f) => f.face.postscript === postscript) ?? preferredFont(fonts);
  const [sample, setSample] = useState(t.sample);
  const [size, setSize] = useState(30);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [plan, setPlan] = useState<UninstallPlan | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const preview = useFontPreview(selected);
  useEffect(() => {
    if (plan && dialog.current && !dialog.current.open)
      dialog.current.showModal();
  }, [plan]);
  if (!selected)
    return (
      <button className="back-button" onClick={onBack}>
        {t.back}
      </button>
    );
  const face = selected.face;
  const date = new Intl.DateTimeFormat(language, {
    dateStyle: "medium",
  }).format(new Date(selected.installedAt * 1000));
  const action = async (work: () => Promise<unknown>) => {
    setError("");
    try {
      await work();
    } catch (e) {
      setError(message(String(e), t));
    }
  };
  const prepare = async () => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      setPlan(await invoke<UninstallPlan>("plan_uninstall", { family }));
    } catch (e) {
      setError(message(String(e), t));
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    if (!plan || busy) return;
    setBusy(true);
    setError("");
    try {
      const outcome = await invoke<UninstallOutcome>("uninstall_family", {
        family: plan.family,
        fingerprint: plan.fingerprint,
        confirmed: true,
      });
      setPlan(null);
      onSnapshot(outcome.snapshot);
      if (outcome.error) setError(message(outcome.error, t));
      else {
        onNotice(t.uninstalledNotice);
        onBack();
      }
    } catch (e) {
      setPlan(null);
      setError(message(String(e), t));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="detail-view native-detail">
      <button className="back-button" onClick={onBack}>
        <Icon name="back" />
        {t.back}
      </button>
      <div className="section-title">
        <h1>{family}</h1>
        <span>{face.variable ? t.variable : t.static}</span>
      </div>
      {(face.foundry || face.designer) && (
        <p className="subtle">
          {[face.foundry, face.designer]
            .filter((s, i, a) => s && a.indexOf(s) === i)
            .join(" · ")}
        </p>
      )}
      <label className="style-select-label">
        {t.style}
        <select
          aria-label={t.chooseStyle}
          value={selected.face.postscript}
          onChange={(e) => setPostscript(e.target.value)}
        >
          {fonts.map((f) => (
            <option key={f.face.postscript} value={f.face.postscript}>
              {f.face.style}
              {f.face.variable ? ` · ${t.variable}` : ""}
            </option>
          ))}
        </select>
      </label>
      <label className="preview-label" htmlFor="native-font-preview">
        {t.realPreview}
      </label>
      {preview.status === "ready" ? (
        <textarea
          id="native-font-preview"
          aria-label={t.previewText}
          value={sample}
          onChange={(e) => setSample(e.target.value)}
          spellCheck={false}
          style={{
            fontFamily: `"${preview.family}"`,
            fontSize: size,
            fontSynthesis: "none",
          }}
        />
      ) : (
        <div className="preview-state" role="status">
          {preview.status === "error"
            ? message(preview.error, t)
            : t.loadingPreview}
        </div>
      )}
      <label className="slider-label">
        {t.size}
        <input
          aria-label={t.size}
          type="range"
          min="16"
          max="72"
          value={size}
          onChange={(e) => setSize(Number(e.target.value))}
        />
        <span>{size}px</span>
      </label>
      {face.axes.length > 0 && (
        <p className="axis-info">{face.axes.join(" · ")}</p>
      )}
      <div className="font-actions">
        <button
          className="secondary-button"
          onClick={() =>
            void action(async () => {
              await navigator.clipboard.writeText(family);
              onNotice(t.copied);
            })
          }
        >
          <Icon name="copy" />
          {t.copyName}
        </button>
        <button
          className="secondary-button"
          onClick={() =>
            void action(() =>
              invoke("reveal_font", { postscript: face.postscript }),
            )
          }
        >
          {t.revealFile}
          <Icon name="arrow" size={12} />
        </button>
      </div>
      <details className="font-metadata">
        <summary>{t.fontDetails}</summary>
        <dl>
          <dt>{t.postscriptName}</dt>
          <dd>{face.postscript}</dd>
          <dt>{t.version}</dt>
          <dd>{face.version || "—"}</dd>
          <dt>{t.installedOn}</dt>
          <dd>{date}</dd>
          <dt>{t.source}</dt>
          <dd>
            {sourceDomain(selected.sourceUrl) || selected.sourceArchive || "—"}
          </dd>
          <dt>{t.file}</dt>
          <dd title={selected.path}>{fileName(selected.path)}</dd>
          <dt>{t.allStyles}</dt>
          <dd>
            {fonts
              .map(
                (f) =>
                  `${f.face.style}${f.face.variable ? ` (${t.variable})` : ""}`,
              )
              .join(" · ")}
          </dd>
        </dl>
      </details>
      <details className="font-license" key={`${face.postscript}-license`}>
        <summary>
          {t.license}
          {!face.license && !face.licenseUrl && (
            <span className="unknown-license">{t.licenseUnknown}</span>
          )}
        </summary>
        {face.license ? (
          <p className="license-text">{face.license}</p>
        ) : (
          <p className="subtle">{t.noLicenseText}</p>
        )}
        {face.licenseUrl && (
          <button
            className="secondary-button"
            onClick={() =>
              void action(() =>
                invoke("open_font_license", { postscript: face.postscript }),
              )
            }
          >
            {t.openLicense}
            <Icon name="arrow" size={12} />
          </button>
        )}
        <p className="license-caption">{t.licenseMetadataHint}</p>
      </details>
      {error && (
        <p className="catch-error" role="alert">
          {error}
        </p>
      )}
      <button
        className="uninstall-button"
        disabled={busy}
        onClick={() => void prepare()}
      >
        {busy ? t.preparingUninstall : t.uninstallFamily}
      </button>
      {plan && (
        <dialog
          ref={dialog}
          className="uninstall-dialog"
          aria-labelledby="uninstall-title"
          onCancel={(e) => {
            e.preventDefault();
            if (!busy) setPlan(null);
          }}
        >
          <h2 id="uninstall-title">{t.uninstallTitle}</h2>
          <p className="uninstall-family">{plan.families.join(" + ")}</p>
          <p>
            {plan.fontCount} {plan.fontCount === 1 ? t.style : t.styles} ·{" "}
            {plan.fileCount} {plan.fileCount === 1 ? t.file : t.files}
          </p>
          {plan.families.length > 1 && (
            <p className="catch-warning">{t.sharedCollectionWarning}</p>
          )}
          <p>{t.uninstallExplanation}</p>
          <div className="confirm-actions">
            <button
              className="secondary-button"
              autoFocus
              disabled={busy}
              onClick={() => setPlan(null)}
            >
              {t.cancel}
            </button>
            <button
              className="primary-button"
              disabled={busy}
              onClick={() => void remove()}
            >
              {busy ? t.uninstalling : t.confirmUninstall}
            </button>
          </div>
        </dialog>
      )}
    </div>
  );
}
