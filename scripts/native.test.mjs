import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const launcher = fileURLToPath(new URL("./native.mjs", import.meta.url));

test("Tauri launches from paths with spaces and preserves literal arguments", () => {
  const root = mkdtempSync(join(tmpdir(), "Fontcatch project & "));
  try {
    const cli = join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
    mkdirSync(dirname(cli), { recursive: true });
    writeFileSync(
      cli,
      "process.stdout.write(JSON.stringify(process.argv.slice(2)));\n",
    );
    const args = [
      "build",
      "--config",
      "folder with spaces/config.json",
      "A&B",
      "$HOME",
      "(literal)",
    ];
    const child = spawnSync(process.execPath, [launcher, "tauri", ...args], {
      cwd: root,
      encoding: "utf8",
    });
    assert.equal(child.status, 0, child.stderr);
    assert.deepEqual(JSON.parse(child.stdout), args);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("native tools preserve their exit code without a shell", () => {
  const child = spawnSync(
    process.execPath,
    [launcher, process.execPath, "-e", "process.exit(7)"],
    {
      encoding: "utf8",
    },
  );
  assert.equal(child.status, 7, child.stderr);
});

test("missing tools fail with an explicit error", () => {
  const child = spawnSync(
    process.execPath,
    [launcher, "fontcatch-missing-test-tool"],
    {
      encoding: "utf8",
    },
  );
  assert.equal(child.status, 1);
  assert.match(child.stderr, /ENOENT/);
});
