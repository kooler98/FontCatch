import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve, delimiter } from "node:path";
const [tool, ...args] = process.argv.slice(2);
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
const executable =
  tool === "tauri"
    ? resolve(
        "node_modules/.bin/tauri" +
          (process.platform === "win32" ? ".cmd" : ""),
      )
    : tool;
const child = spawn(executable, args, {
  env,
  stdio: "inherit",
  shell: process.platform === "win32",
});
child.on("error", (e) => {
  console.error(e.message);
  process.exit(1);
});
child.on("exit", (code) => process.exit(code ?? 1));
