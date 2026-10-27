export type Language = "fr" | "en";
export type Tab = "recent" | "library" | "settings";
export interface Settings {
  mode: "ask" | "auto";
  fontPreference: "variable" | "static" | "both";
  trashAfterInstall: boolean;
  language: Language;
  folders: string[];
}
export interface Family {
  id: string;
  name: string;
  foundry: string;
  source: string;
  sources?: string[];
  styles: string[];
  variable: boolean;
  hasStatic?: boolean;
  status: "pending" | "installed" | "ignored" | "failed";
  preview: "sans-serif" | "serif" | "monospace";
}
export const defaultSettings: Settings = {
  mode: "auto",
  fontPreference: "variable",
  trashAfterInstall: false,
  language: "en",
  folders: [],
};
