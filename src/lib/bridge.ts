import { invoke, isTauri } from "@tauri-apps/api/core";
import { defaultSettings, type Settings } from "./types";
import config from "../config.json";
export const native = isTauri();
const PREVIEW_KEY = "fontcatch-design-preview-settings";
export async function readSettings(): Promise<Settings> {
  if (native) return invoke("get_settings");
  const saved = localStorage.getItem(PREVIEW_KEY);
  return saved
    ? { ...defaultSettings, ...JSON.parse(saved) }
    : { ...defaultSettings };
}
export async function saveSettings(settings: Settings): Promise<Settings> {
  if (native) return invoke("save_settings", { settings });
  localStorage.setItem(PREVIEW_KEY, JSON.stringify(settings));
  return settings;
}
export async function openLink(key: keyof typeof config) {
  if (native) return invoke("open_link", { key });
  const url = config[key] as string | null;
  if (url?.startsWith("https://"))
    window.open(url, "_blank", "noopener,noreferrer");
}
export async function hidePanel() {
  if (native) await invoke("hide_panel");
}
export async function chooseFolder(): Promise<string | null> {
  if (!native) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  await invoke("set_dialog_active", { active: true });
  try {
    return await open({ directory: true, multiple: false });
  } finally {
    await invoke("set_dialog_active", { active: false });
  }
}

export async function readSnapshot(): Promise<import("./native").Snapshot> {
  return invoke("get_snapshot");
}
export async function installFont(
  id: string,
  confirmUpdate: boolean,
): Promise<import("./native").Snapshot> {
  return invoke("install_font", { id, confirmUpdate });
}
export async function ignoreFont(
  id: string,
): Promise<import("./native").Snapshot> {
  return invoke("ignore_font", { id });
}
