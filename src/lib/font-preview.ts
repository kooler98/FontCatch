import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InstalledFont } from "./native";

type Preview = {
  family: string;
  status: "loading" | "ready" | "error";
  error: string;
};
type Entry = { refs: number; face: FontFace | null; promise: Promise<string> };
const loaded = new Map<string, Entry>();
let sequence = 0;
export function fontKey(font: InstalledFont) {
  return `${font.face.postscript}:${font.hash}:${font.face.faceIndex}`;
}
function acquire(font: InstalledFont) {
  const key = fontKey(font);
  let entry = loaded.get(key);
  if (!entry) {
    const current: Entry = {
      refs: 0,
      face: null,
      promise: Promise.resolve(""),
    };
    const family = `fontcatch_preview_${++sequence}`;
    current.promise = invoke<ArrayBuffer | number[]>("read_font_preview", {
      postscript: font.face.postscript,
    }).then(async (bytes) => {
      const buffer =
        bytes instanceof ArrayBuffer ? bytes : new Uint8Array(bytes).buffer;
      // FontFace is the programmatic @font-face interface. The backend supplies
      // only the verified installed resource, never an arbitrary filesystem path.
      const face = await new FontFace(family, buffer).load();
      current.face = face;
      if (current.refs > 0) document.fonts.add(face);
      return family;
    });
    loaded.set(key, current);
    entry = current;
  }
  entry.refs++;
  const held = entry;
  return {
    promise: held.promise,
    release: () => {
      held.refs--;
      if (held.refs === 0) {
        if (held.face) document.fonts.delete(held.face);
        if (loaded.get(key) === held) loaded.delete(key);
      }
    },
  };
}
export function useFontPreview(
  font: InstalledFont | undefined,
  enabled = true,
): Preview {
  const key = font ? fontKey(font) : "";
  const [state, setState] = useState<Preview & { key: string }>({
    key: "",
    family: "",
    status: "loading",
    error: "",
  });
  useEffect(() => {
    if (!font || !enabled) return;
    let cancelled = false;
    setState({ key, family: "", status: "loading", error: "" });
    const entry = acquire(font);
    entry.promise.then(
      (family) => {
        if (!cancelled) setState({ key, family, status: "ready", error: "" });
      },
      (error) => {
        if (!cancelled)
          setState({
            key,
            family: "",
            status: "error",
            error: typeof error === "string" ? error : "font_preview_failed",
          });
      },
    );
    return () => {
      cancelled = true;
      entry.release();
    };
  }, [key, enabled]);
  return enabled && state.key === key
    ? state
    : { family: "", status: "loading", error: "" };
}
export function preferredFont(fonts: InstalledFont[]) {
  return (
    fonts.find((f) => /^(regular|normal|book)$/i.test(f.face.style)) ??
    fonts.find((f) => f.face.variable) ??
    fonts[0]
  );
}
