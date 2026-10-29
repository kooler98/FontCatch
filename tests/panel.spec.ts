import { test, expect } from "@playwright/test";
test("demo approval, library filters, preview and empty state", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(
    page.getByText("Simulation · no fonts are installed"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Install (demo)" }).click();
  await expect(page.getByText("Inspecting typeface…")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Install (demo)" }),
  ).toHaveCount(0);
  await expect(page.locator(".task-row")).toHaveCount(4);
  await page.getByRole("tab", { name: "Library" }).click();
  await expect(
    page.getByRole("button", { name: "Convert file…" }),
  ).toBeDisabled();
  await expect(page.getByText("Available in the desktop app")).toBeVisible();
  await expect(page.locator(".library-card")).toHaveCount(4);
  await page
    .getByRole("textbox", { name: "Name, foundry or source…" })
    .fill("pangram");
  await expect(page.locator(".library-card")).toHaveCount(1);
  await page.getByRole("button", { name: "Clear search" }).click();
  await page.getByRole("button", { name: "Static", exact: true }).click();
  await page
    .getByLabel("Source", { exact: true })
    .selectOption("fonts.google.com");
  await expect(page.locator(".library-card")).toHaveCount(1);
  await page.locator(".library-card").click();
  await page
    .getByRole("textbox", { name: "Preview text" })
    .fill("Hello Fontcatch");
  await expect(page.getByRole("textbox", { name: "Preview text" })).toHaveValue(
    "Hello Fontcatch",
  );
  await page.getByRole("slider").fill("42");
  await expect(page.getByText("42px")).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Empty state", exact: true }).click();
  await page.getByRole("tab", { name: "Recent" }).click();
  await expect(page.getByText("No fonts detected")).toBeVisible();
  await expect(page.locator(".panel")).toHaveCSS("width", "360px");
  await expect(page.locator(".panel")).toHaveCSS("height", "480px");
  expect(errors).toEqual([]);
});
test("settings persist, language updates, keyboard tabs and honest unavailable controls", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "Settings" }).click();
  await expect(
    page.getByRole("button", { name: "Automatic", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Ask me", exact: true }).click();
  await page.reload();
  await page.getByRole("tab", { name: "Settings" }).click();
  await expect(
    page.getByRole("button", { name: "Ask me", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Automatic", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Automatic", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("switch", { name: "Move the zip to Trash" }).click();
  await expect(
    page.getByRole("switch", { name: "Move the zip to Trash" }),
  ).toHaveAttribute("aria-checked", "true");
  await page.getByLabel("Language", { exact: true }).selectOption("fr");
  await expect(page.getByRole("tab", { name: "Réglages" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("lang", "fr");
  await page.reload();
  await page.getByRole("tab", { name: "Réglages" }).click();
  await expect(page.getByLabel("Langue", { exact: true })).toHaveValue("fr");
  await expect(
    page.getByRole("button", { name: "Automatique", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByLabel("Langue", { exact: true }).selectOption("en");
  await expect(page.getByRole("tab", { name: "Settings" })).toBeVisible();
  await page.reload();
  await page.getByRole("tab", { name: "Settings" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.getByLabel("Language", { exact: true })).toHaveValue("en");
  await expect(
    page.getByRole("button", { name: "Automatic", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByRole("switch", { name: "Move the zip to Trash" }),
  ).toHaveAttribute("aria-checked", "true");
  await expect(
    page.getByRole("switch", { name: "Launch at login" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Buy me a coffee" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: /About/ }).click();
  await expect(
    page.getByText("Data stored locally. No usage tracking."),
  ).toBeVisible();
  await page.getByRole("tab", { name: "Settings" }).focus();
  await page.keyboard.press("Home");
  await expect(page.getByRole("tab", { name: "Recent" })).toBeFocused();
  await expect(page.getByRole("tab", { name: "Recent" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});
test("screens fit without horizontal overflow at actual panel size", async ({
  page,
}) => {
  await page.goto("/");
  for (const tab of ["Recent", "Library", "Settings"]) {
    await page.getByRole("tab", { name: tab }).click();
    expect(
      await page
        .locator(".panel-content")
        .evaluate((el) => el.scrollWidth <= el.clientWidth),
    ).toBe(true);
  }
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
