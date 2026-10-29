import { useId, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { native } from "../lib/bridge";
import type { Woff2Export } from "../lib/native";
import type { Language } from "../lib/types";
import type fr from "../locales/fr.json";

export function ExportWoff2Button({
  postscript,
  styleName,
  t,
  language,
  disabled = false,
  onNotice,
  onBusyChange,
}: {
  postscript: string | null;
  styleName?: string;
  t: typeof fr;
  language: Language;
  disabled?: boolean;
  onNotice: (text: string) => void;
  onBusyChange?: (busy: boolean) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const lock = useRef(false);
  const hintId = useId();
  const label = postscript === null ? t.convertFile : t.exportWoff2;
  const convert = async () => {
    if (!native || disabled || lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    onBusyChange?.(true);
    try {
      const result = await invoke<Woff2Export | null>("export_woff2", {
        postscript,
      });
      if (result) {
        const size = new Intl.NumberFormat(language, {
          style: "unit",
          unit: result.outputBytes < 1000 ? "byte" : "kilobyte",
          maximumFractionDigits: 1,
        }).format(
          result.outputBytes < 1000
            ? result.outputBytes
            : result.outputBytes / 1000,
        );
        onNotice(
          t.woff2Saved
            .replace("{fileName}", result.fileName)
            .replace("{size}", size),
        );
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      lock.current = false;
      setBusy(false);
      onBusyChange?.(false);
    }
  };
  return (
    <div className="woff2-export">
      <div className="woff2-action">
        <button
          className="secondary-button"
          disabled={!native || disabled || busy}
          aria-busy={busy}
          aria-label={styleName ? `${label} — ${styleName}` : label}
          aria-describedby={hintId}
          onClick={() => void convert()}
        >
          {busy ? t.convertingWoff2 : label}
        </button>
        <span className="woff2-context">
          {styleName ?? "TTF / OTF → WOFF2"}
        </span>
      </div>
      <p id={hintId} className="woff2-hint">
        {!native
          ? t.nativeOnly
          : postscript === null
            ? t.woff2FileHint
            : t.woff2LicenseHint}
      </p>
      {error && (
        <p className="catch-error" role="alert">
          {Object.hasOwn(t, error)
            ? (t as Record<string, string>)[error]
            : t.woff2_conversion_failed}
        </p>
      )}
    </div>
  );
}
