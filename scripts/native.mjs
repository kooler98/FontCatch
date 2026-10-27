import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve, delimiter } from "node:path";
const [tool, ...args] = process.argv.slice(2);
if (!tool) {
  console.error("Usage: node scripts/native.mjs <tool> [arguments...]");
  process.exit(1);
}
const env = { ...process.env };
if (existsSync(".tooling/cargo/bin")) {
  env.CARGO_HOME = resolve(".tooling/cargo");
  env.RUSTUP_HOME = resolve(".tooling/rustup");
  env.PATH = `${resolve(".tooling/cargo/bin")}${delimiter}${env.PATH}`;
}
if (
  process.platform === "darwin" &&
  !env.SDKROOT &&
  existsSync(".tooling/sdk-path")
) {
  env.SDKROOT = readFileSync(".tooling/sdk-path", "utf8").trim();
}
const executable = tool === "tauri" ? process.execPath : tool;
// Run Tauri's JavaScript entry directly. Windows .cmd wrappers otherwise need a
// shell, which breaks project paths with spaces and interprets argument syntax.
const childArgs =
  tool === "tauri"
    ? [resolve("node_modules/@tauri-apps/cli/tauri.js"), ...args]
    : args;
const child = spawn(executable, childArgs, {
  env,
  stdio: "inherit",
  shell: false,
});
child.on("error", (e) => {
  console.error(e.message);
  process.exit(1);
});
child.on("exit", (code) => process.exit(code ?? 1));
