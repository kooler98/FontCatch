import { describe, it, expect } from "vitest";
import { demoFamilies, filterFamilies } from "./library";
import fr from "../locales/fr.json";
import en from "../locales/en.json";
describe("Library search", () => {
  it("never includes pending or ignored families", () => {
    expect(
      filterFamilies(demoFamilies, "", "all", "").map((f) => f.id),
    ).toEqual(["manrope", "editorial", "mono"]);
  });
  it("combines source and static filters", () => {
    expect(
      filterFamilies(demoFamilies, "", "static", "fonts.google.com").map(
        (f) => f.id,
      ),
    ).toEqual(["mono"]);
  });
  it("searches foundries and trims whitespace", () => {
    expect(filterFamilies(demoFamilies, "  PANGRAM ", "all", "")[0]?.id).toBe(
      "editorial",
    );
  });
  it("normalizes accents for designer names", () => {
    expect(
      filterFamilies(
        [{ ...demoFamilies[1], foundry: "Émilie" }],
        "emilie",
        "all",
        "",
      ),
    ).toHaveLength(1);
  });
  it("returns an empty result for any unmatched query length", () => {
    expect(filterFamilies(demoFamilies, "z", "all", "")).toEqual([]);
  });
});
it("ships identical translation keys in both languages", () => {
  expect(Object.keys(fr).sort()).toEqual(Object.keys(en).sort());
});

describe("Mixed native families", () => {
  const family = {
    ...demoFamilies[1],
    variable: true,
    hasStatic: true,
    sources: ["one.example", "two.example"],
    source: "one.example",
  };
  it("keeps a mixed family in both static and variable filters", () => {
    expect(filterFamilies([family], "", "static", "")).toHaveLength(1);
    expect(filterFamilies([family], "", "variable", "")).toHaveLength(1);
  });
  it("matches every source in the same family", () => {
    expect(
      filterFamilies([family], "two.example", "all", "two.example"),
    ).toHaveLength(1);
  });
});
