import { test, expect, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
test.use({ viewport: { width: 360, height: 480 } });
const regularBytes = [
  ...readFileSync("src-tauri/tests/fixtures/FontcatchQA-Regular.ttf"),
];
const boldBytes = [
  ...readFileSync("src-tauri/tests/fixtures/FontcatchQA-Bold.ttf"),
];

// Real FontFace loading in the browser, with only the native IPC transport mocked.
async function mockNative(
  page: Page,
  options: {
    installed?: boolean;
    shared?: boolean;
    corrupt?: boolean;
    stale?: boolean;
    historyCount?: number;
  } = {},
) {
  await page.addInitScript(
    ({ regularBytes, boldBytes, options }) => {
      const w = window as any;
      w.isTauri = true;
      let next = 0;
      const callbacks: Record<number, (e: any) => void> = {};
      const events: Record<number, { event: string; handler: number }> = {};
      const face = {
        family: "Fontcatch QA",
        style: "Regular",
        postscript: "FontcatchQA-Regular",
        version: "Version 1.000",
        revision: 65536,
        foundry: "Fontcatch QA",
        designer: "Fontcatch",
        license: "Test license <b>plain text</b>",
        licenseUrl: "https://example.org/license",
        variable: false,
        faceIndex: 0,
        axes: [],
      };
      const bold = {
        ...face,
        style: "Bold",
        postscript: "FontcatchQA-Bold",
        license: null,
        licenseUrl: null,
      };
      const now = Math.floor(Date.now() / 1000);
      const record = (face: any, hash: string) => ({
        face,
        path: `/fonts/${hash}.ttf`,
        hash,
        installedAt: now,
        sourceArchive: "typeface.zip",
        sourceUrl: "https://fonts.example.org/typeface.zip",
      });
      const fonts = [record(face, "regular"), record(bold, "bold")];
      const job: any = {
        id: "qa-job",
        sourcePath: "/qa/typeface.zip",
        sourceUrl: fonts[0].sourceUrl,
        createdAt: now,
        updatedAt: now,
        status: options.installed ? "installed" : "pending",
        files: [
          { path: "/cache/Regular.ttf", hash: "regular", faces: [face] },
          { path: "/cache/Bold.ttf", hash: "bold", faces: [bold] },
        ],
        warnings: [],
        error: null,
        requiresUpdate: true,
        installedCount: options.installed ? 2 : 0,
        removedCount: 0,
        skippedCount: 0,
        planned: [],
      };
      const snapshot: any = {
        recent: options.historyCount
          ? Array.from({ length: options.historyCount }, (_, i) => ({
              ...job,
              id: `history-${i}`,
              status: "installed",
            }))
          : [job],
        installed: options.installed ? fonts : [],
        processing: null,
        watcherErrors: [],
      };
      w.__fontcatchTestCalls = [];
      const emit = () => {
        for (const [id, listener] of Object.entries(events))
          if (listener.event === "library-changed")
            callbacks[listener.handler]({
              event: "library-changed",
              id: Number(id),
              payload: structuredClone(snapshot),
            });
      };
      w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: (_event: string, id: number) => delete events[id],
      };
      w.__TAURI_INTERNALS__ = {
        transformCallback: (fn: any) => {
          callbacks[++next] = fn;
          return next;
        },
        invoke: async (cmd: string, args: any) => {
          w.__fontcatchTestCalls.push({ cmd, args });
          if (cmd === "get_settings")
            return {
              mode: "ask",
              fontPreference: "variable",
              trashAfterInstall: false,
              language: "en",
              folders: ["/qa"],
            };
          if (
            cmd === "plugin:autostart|is_enabled" ||
            cmd === "plugin:notification|is_permission_granted"
          )
            return false;
          if (cmd === "plugin:event|listen") {
            events[++next] = args;
            return next;
          }
          if (cmd === "get_snapshot") return structuredClone(snapshot);
          if (cmd === "read_font_preview")
            return options.corrupt
              ? [1, 2, 3]
              : args.postscript.endsWith("Bold")
                ? boldBytes
                : regularBytes;
          if (cmd === "install_font") {
            if (!args.confirmUpdate) throw "update_confirmation_required";
            job.status = "installed";
            job.installedCount = 2;
            snapshot.installed = fonts;
            emit();
            return structuredClone(snapshot);
          }
          if (cmd === "plan_uninstall")
            return {
              family: "Fontcatch QA",
              fingerprint: "confirmed-revision",
              families: options.shared
                ? ["Fontcatch Other", "Fontcatch QA"]
                : ["Fontcatch QA"],
              fontCount: 2,
              fileCount: options.shared ? 1 : 2,
            };
          if (cmd === "uninstall_family") {
            if (!args.confirmed || args.fingerprint !== "confirmed-revision")
              throw "uninstall_confirmation_required";
            if (options.stale) throw "library_changed_review";
            snapshot.installed = [];
            job.status = "uninstalled";
            job.removedCount = 2;
            emit();
            return {
              snapshot: structuredClone(snapshot),
              removedCount: 2,
              error: null,
            };
          }
          return null;
        },
      };
    },
    { regularBytes, boldBytes, options },
  );
}
const calls = (page: Page, command: string) =>
  page.evaluate(
    (cmd) =>
      (window as any).__fontcatchTestCalls.filter((c: any) => c.cmd === cmd),
    command,
  );
test("large histories mount only the visible batch and expanded details", async ({
  page,
}) => {
  await mockNative(page, { installed: true, historyCount: 130 });
  await page.goto("/");
  const appUrl = page.url();
  const credit = page.getByRole("link", { name: "Made by @pierrreweb" });
  await expect(credit).toBeVisible();
  await expect(credit).toHaveAttribute(
    "href",
    "https://twitter.com/pierrreweb",
  );
  await credit.click();
  expect(await calls(page, "open_link")).toEqual([
    { cmd: "open_link", args: { key: "creatorUrl" } },
  ]);
  await expect(page).toHaveURL(appUrl);
  await expect(page.locator(".task-row")).toHaveCount(50);
  await expect(page.locator(".task-detail")).toHaveCount(0);
  await page.locator(".task-trigger").first().click();
  await expect(page.locator(".task-detail")).toHaveCount(1);
  await page.getByRole("button", { name: "Show more" }).click();
  await expect(page.locator(".task-row")).toHaveCount(100);
  await page.getByRole("button", { name: "Show more" }).click();
  await expect(page.locator(".task-row")).toHaveCount(130);
  await expect(page.getByRole("button", { name: "Show more" })).toHaveCount(0);
});
async function openFamily(page: Page) {
  await page.getByRole("tab", { name: "Library" }).click();
  await page.locator(".library-card").click();
}
async function previewWidth(page: Page) {
  return page.getByRole("textbox", { name: "Preview text" }).evaluate((el) => {
    const ctx = document.createElement("canvas").getContext("2d")!;
    ctx.font = `30px ${getComputedStyle(el).fontFamily}`;
    return ctx.measureText("AAA").width;
  });
}

test("native update, real previews by style, license metadata and file actions", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await mockNative(page);
  await page.goto("/");
  await expect(page.getByText("NEW VERSION")).toBeVisible();
  await page.getByRole("button", { name: "Update", exact: true }).click();
  await expect(page.locator(".task-row")).toContainText("Installed");
  expect(await calls(page, "install_font")).toEqual([
    { cmd: "install_font", args: { id: "qa-job", confirmUpdate: true } },
  ]);
  await page.getByRole("tab", { name: "Library" }).click();
  await expect(page.locator(".real-sample")).toHaveClass(/ready/);
  await page.locator(".library-card").click();
  const preview = page.getByRole("textbox", { name: "Preview text" });
  await expect(preview).toBeVisible();
  await expect(preview).toHaveCSS("font-family", /fontcatch_preview_/);
  const regularWidth = await previewWidth(page);
  await preview.fill("AAA");
  await page.getByRole("slider", { name: "Size" }).fill("42");
  await expect(preview).toHaveCSS("font-size", "42px");
  await page.locator(".font-license summary").click();
  await expect(page.locator(".license-text")).toHaveText(
    "Test license <b>plain text</b>",
  );
  await expect(page.locator(".license-text b")).toHaveCount(0);
  await page.getByRole("button", { name: "Read the license" }).click();
  expect((await calls(page, "open_font_license"))[0].args.postscript).toBe(
    "FontcatchQA-Regular",
  );
  await page.getByRole("button", { name: "Reveal file" }).click();
  expect((await calls(page, "reveal_font"))[0].args.postscript).toBe(
    "FontcatchQA-Regular",
  );
  await page.getByLabel("Choose a style").selectOption("FontcatchQA-Bold");
  await expect(preview).toBeVisible();
  await expect(preview).toHaveValue("AAA");
  expect(await previewWidth(page)).toBeGreaterThan(regularWidth);
  await expect(
    page.getByText("Unknown license", { exact: true }),
  ).toBeVisible();
  expect(
    await page
      .locator(".panel-content")
      .evaluate((el) => el.scrollWidth <= el.clientWidth),
  ).toBe(true);
  expect(errors).toEqual([]);
});

test("native removal requires confirmation, cancels with Escape, then updates the library", async ({
  page,
}) => {
  await mockNative(page, { installed: true });
  await page.goto("/");
  await openFamily(page);
  await page.getByRole("button", { name: "Uninstall family…" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  expect(await calls(page, "uninstall_family")).toHaveLength(0);
  await page.getByRole("button", { name: "Uninstall family…" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.locator(".native-detail")).toBeVisible();
  await page.getByRole("button", { name: "Uninstall family…" }).click();
  await page
    .getByRole("button", { name: "Move to Trash", exact: true })
    .click();
  await expect(page.getByText("No fonts installed")).toBeVisible();
  expect(await calls(page, "uninstall_family")).toEqual([
    {
      cmd: "uninstall_family",
      args: {
        family: "Fontcatch QA",
        fingerprint: "confirmed-revision",
        confirmed: true,
      },
    },
  ]);
});

test("shared TTC confirmation names every family and fits the panel", async ({
  page,
}) => {
  await mockNative(page, { installed: true, shared: true });
  await page.goto("/");
  await openFamily(page);
  await page.getByRole("button", { name: "Uninstall family…" }).click();
  const modal = page.getByRole("dialog");
  await expect(modal).toContainText("Fontcatch Other + Fontcatch QA");
  await expect(modal).toContainText("will be removed together");
  const box = await modal.boundingBox();
  expect(box!.width).toBeLessThanOrEqual(328);
  expect(box!.x).toBeGreaterThanOrEqual(16);
  expect(box!.y).toBeGreaterThanOrEqual(24);
  expect(box!.x + box!.width).toBeLessThanOrEqual(344);
  expect(box!.y + box!.height).toBeLessThanOrEqual(456);
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  expect(await calls(page, "uninstall_family")).toHaveLength(0);
});

test("failed previews remain explicit and stale uninstall consent cannot remove fonts", async ({
  page,
}) => {
  await mockNative(page, { installed: true, corrupt: true, stale: true });
  await page.goto("/");
  await openFamily(page);
  await expect(
    page.getByText("This font cannot be displayed in the preview."),
  ).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Preview text" })).toHaveCount(
    0,
  );
  await page.getByRole("button", { name: "Uninstall family…" }).click();
  await page
    .getByRole("button", { name: "Move to Trash", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText("The library changed.");
  await expect(page.locator(".native-detail h1")).toHaveText("Fontcatch QA");
});
