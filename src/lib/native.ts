import type { Family } from "./types";
export interface FontFace {
  family: string;
  style: string;
  postscript: string;
  version: string;
  revision: number;
  foundry: string;
  designer: string;
  license: string | null;
  licenseUrl: string | null;
  variable: boolean;
  faceIndex: number;
  axes: string[];
}
export interface FontFile {
  path: string;
  hash: string;
  faces: FontFace[];
}
export interface CatchJob {
  id: string;
  sourcePath: string;
  sourceUrl: string | null;
  createdAt: number;
  updatedAt: number;
  status:
    | "pending"
    | "installing"
    | "installed"
    | "ignored"
    | "failed"
    | "uninstalling"
    | "uninstalled"
    | "uninstall_failed";
  files: FontFile[];
  warnings: string[];
  error: string | null;
  requiresUpdate: boolean;
  installedCount: number;
  skippedCount: number;
  removedCount: number;
  planned: FontFile[];
}
export interface InstalledFont {
  face: FontFace;
  path: string;
  hash: string;
  installedAt: number;
  sourceArchive: string;
  sourceUrl: string | null;
}
export interface Snapshot {
  recent: CatchJob[];
  installed: InstalledFont[];
  processing: string | null;
  watcherErrors: string[];
}
export interface UninstallPlan {
  family: string;
  fingerprint: string;
  families: string[];
  fontCount: number;
  fileCount: number;
}
export interface UninstallOutcome {
  snapshot: Snapshot;
  removedCount: number;
  error: string | null;
}
export const emptySnapshot: Snapshot = {
  recent: [],
  installed: [],
  processing: null,
  watcherErrors: [],
};
export function sourceDomain(url: string | null) {
  if (!url) return "";
  try {
    return new URL(url).hostname;
  } catch {
    return "";
  }
}
export function fileName(path: string) {
  return path.split(/[\\/]/).at(-1) ?? path;
}
export function familiesFromSnapshot(snapshot: Snapshot): Family[] {
  const groups = new Map<string, Family>();
  for (const font of snapshot.installed) {
    let f = groups.get(font.face.family);
    if (!f) {
      f = {
        id: font.face.family,
        name: font.face.family,
        foundry: font.face.foundry || font.face.designer,
        source: sourceDomain(font.sourceUrl) || font.sourceArchive,
        sources: [],
        styles: [],
        variable: false,
        hasStatic: false,
        status: "installed",
        preview: "sans-serif",
      };
      groups.set(f.name, f);
    }
    f.styles.push(font.face.style);
    f.variable ||= font.face.variable;
    f.hasStatic ||= !font.face.variable;
    const source = sourceDomain(font.sourceUrl) || font.sourceArchive;
    if (source && !f.sources!.includes(source)) f.sources!.push(source);
    for (const name of [font.face.foundry, font.face.designer]) {
      if (name && !f.foundry.split(" · ").includes(name))
        f.foundry = [f.foundry, name].filter(Boolean).join(" · ");
    }
  }
  return [...groups.values()];
}
export function relativeTime(timestamp: number, locale: "fr" | "en") {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - timestamp));
  const unit = seconds < 3600 ? "minute" : seconds < 86400 ? "hour" : "day";
  const amount =
    unit === "minute"
      ? Math.floor(seconds / 60)
      : unit === "hour"
        ? Math.floor(seconds / 3600)
        : Math.floor(seconds / 86400);
  return new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(
    -amount,
    unit,
  );
}
