import { readFile, writeFile, mkdir } from "node:fs/promises";
import sharp from "sharp";
const svg = await readFile("src/assets/brand/fontcatch.svg", "utf8");
const claw = svg.match(/<path\b[^>]*fill="#D3A9FF"[^>]*\/>/i)?.[0];
if (!claw) throw new Error("Exact lilac logo path not found");
const wrap = (body, viewBox = "0 0 188 188") =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}">${body}</svg>`;
await mkdir("src-tauri/icons/tray", { recursive: true });
await writeFile(
  "src/assets/brand/mark.svg",
  wrap(
    `<rect width="188" height="188" rx="42" fill="#3B1102"/><g transform="translate(0 -10)">${claw}</g>`,
  ),
);
// Apple-style 824 px rounded square centered in a transparent 1024 px canvas.
const app = wrap(
  `<rect x="100" y="100" width="824" height="824" rx="184" fill="#3B1102"/><g transform="translate(100 56.17) scale(4.383)">${claw}</g>`,
  "0 0 1024 1024",
);
await writeFile("src/assets/brand/app-icon.svg", app);
await sharp(Buffer.from(app))
  .resize(1024, 1024)
  .png()
  .toFile("src/assets/brand/app-icon.png");
for (const [name, color] of [
  ["black", "#000000"],
  ["white", "#FFFFFF"],
]) {
  const path = claw.replace(/#D3A9FF/gi, color);
  const tray = wrap(path, "22 40 144 144");
  await writeFile(`src/assets/brand/tray-${name}.svg`, tray);
  for (const size of [18, 22, 32, 44])
    await sharp(Buffer.from(tray))
      .resize(size, size)
      .png()
      .toFile(`src-tauri/icons/tray/${name}-${size}.png`);
}

for (const name of ["black", "white"]) {
  await sharp(`src-tauri/icons/tray/${name}-44.png`)
    .composite([
      {
        input: Buffer.from(
          '<svg width="44" height="44"><circle cx="36" cy="8" r="7" fill="#3B1102"/><circle cx="36" cy="8" r="5" fill="#D3A9FF"/></svg>',
        ),
      },
    ])
    .png()
    .toFile(`src-tauri/icons/tray/${name}-pending-44.png`);
}
