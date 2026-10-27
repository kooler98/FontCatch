import type { Family } from "./types";
export function filterFamilies(
  families: Family[],
  query: string,
  kind: "all" | "variable" | "static",
  source: string,
) {
  const normalized = query
    .trim()
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase();
  return families.filter(
    (f) =>
      f.status === "installed" &&
      (kind === "all" ||
        (kind === "variable" ? f.variable : (f.hasStatic ?? !f.variable))) &&
      (!source || (f.sources ?? [f.source]).includes(source)) &&
      [f.name, f.foundry, ...(f.sources ?? [f.source])].some((v) =>
        v
          .normalize("NFD")
          .replace(/\p{Diacritic}/gu, "")
          .toLocaleLowerCase()
          .includes(normalized),
      ),
  );
}
export const demoFamilies: Family[] = [
  {
    id: "inter",
    name: "Inter",
    foundry: "Rasmus Andersson",
    source: "fonts.google.com",
    styles: ["Regular", "Italic", "Variable · 100–900"],
    variable: true,
    status: "pending",
    preview: "sans-serif",
  },
  {
    id: "manrope",
    name: "Manrope",
    foundry: "Mikhail Sharanda",
    source: "fonts.google.com",
    styles: ["Variable · 200–800"],
    variable: true,
    status: "installed",
    preview: "sans-serif",
  },
  {
    id: "editorial",
    name: "Editorial New",
    foundry: "Pangram Pangram",
    source: "pangrampangram.com",
    styles: ["Regular", "Italic", "Ultrabold"],
    variable: false,
    status: "installed",
    preview: "serif",
  },
  {
    id: "mono",
    name: "Space Mono",
    foundry: "Colophon",
    source: "fonts.google.com",
    styles: ["Regular", "Bold"],
    variable: false,
    status: "installed",
    preview: "monospace",
  },
];
