# Contributing to Fontcatch

Thanks for helping improve Fontcatch. This is an early alpha; small, focused changes and native platform testing are especially useful.

## Report a problem

Open a GitHub issue with the app version, operating system, installation mode, file type, steps to reproduce, and expected/actual behavior. If possible, reproduce with an openly licensed or original synthetic font.

Do not attach purchased fonts, private download links, or your library database. Remove personal paths and source URLs from logs and screenshots before posting them.

## Work locally

Follow the prerequisites and setup in [README.md](README.md), create a branch, and keep each pull request focused on one change. Include a brief explanation of the problem, resulting behavior, and checks performed.

```sh
npm ci
npm run desktop
npm run build
npm test
npm run test:rust
npm run test:ui
```

Run relevant checks for your change. State which operating systems you actually tested; compiling platform-specific source on macOS is not proof of Windows or Linux behavior.

## Project structure

The front end is React, TypeScript, Vite, and Tailwind. UI tokens live in `src/styles/tokens.css`; translations are simple FR/EN JSON files. Beautiful UI components are copied and adapted, with their license kept alongside the source.

The Rust pipeline is split into `watcher`, `extractor`, `inspector`, `installer`, `library`, `preview`, and `engine`. Platform installers share a common trait. Font data uses SQLite; preferences use a separate Tauri Store file.

Maintain these behaviors:

- No account, telemetry, or automatic downloading of fonts.
- Current-user installation only; never replace fonts managed outside Fontcatch.
- Confirm updates even in automatic mode. Preserve an explicit Ignore decision.
- Confirm uninstallation against the current library state. List every family sharing a TTC file.
- Move user files to Trash instead of permanently deleting them; refuse files that changed after installation.
- Preview the actual managed file. Keep missing or unreadable preview errors visible.
- Do not process pre-existing downloads on startup. Resume already recorded pending requests in Auto mode.
- Keep French and English copy consistent.

## Test fixtures and real installation

The small font fixtures in `src-tauri/tests/fixtures/` were generated for this project and use CC0. They are not commercial typefaces. Regenerate them with `scripts/font-fixtures.py` using fontTools 4.60.2.

The default Rust suite uses temporary directories. The following **opt-in macOS test modifies the actual user Fonts folder**: it installs one original synthetic test font, verifies the preview data, then uninstalls it through the engine into the real Trash. It intentionally stays excluded from ordinary tests and CI.

```sh
FONTCATCH_NATIVE_SMOKE=1 node scripts/native.mjs cargo test --manifest-path src-tauri/Cargo.toml macos_native_install_and_trash -- --ignored --nocapture
```

The separate configuration `src-tauri/tauri.qa.conf.json` builds a QA application with its own settings and database. It still uses the real current-user font directory when installation is requested.

```sh
npm run desktop:build -- --debug --bundles app --config src-tauri/tauri.qa.conf.json
```

## Licensing

Submit only code and assets you have the right to contribute. Contributions to Fontcatch are distributed under the repository's MIT license; retain existing third-party notices. Do not add fonts from downloads or purchases as fixtures.
