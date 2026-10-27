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
npm run test:tooling
npm run test:ui
```

Run relevant checks for your change. `npm run test:tooling` checks the native launcher, including project paths containing spaces and literal command arguments. State which operating systems you actually tested; compiling platform-specific source on macOS is not proof of Windows or Linux behavior.

Windows CI runs on Windows Server 2022 x64 with an administrator account. Successful CI checks do not establish Windows 10/11 desktop behavior or standard-user permissions. For Windows changes, record the Windows version and account type, and test the tray, display scaling, folder selection, previews, notifications, launch at login, and installation/removal without elevation.

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

The default Rust suite uses temporary directories and simulated installation. The opt-in tests below modify the real current-user font installation and are excluded from the default suite.

### macOS

The **macOS smoke test modifies the actual user Fonts folder**: it installs one original synthetic test font, verifies the preview data, then uninstalls it through the engine into the real Trash. It intentionally stays excluded from ordinary tests and CI.

```sh
FONTCATCH_NATIVE_SMOKE=1 node scripts/native.mjs cargo test --manifest-path src-tauri/Cargo.toml macos_native_install_and_trash -- --ignored --nocapture
```

### Windows

The **Windows smoke test modifies the current user's font files and font registry entries**. It installs the original CC0 fixture, checks its current-user registry registration and GDI font selection, repeats registration to check recovery, then removes it to the Recycle Bin. It verifies that the registry entries and GDI selection are removed and the source fixture is unchanged. The test refuses to run if the test font is already installed.

Run it in PowerShell on a Windows test machine. An elevated terminal is not required. The Windows CI job opts into this test explicitly; it is still ignored by ordinary `npm run test:rust` runs.

```powershell
$env:FONTCATCH_WINDOWS_SMOKE = "1"
try {
    node scripts/native.mjs cargo test --manifest-path src-tauri/Cargo.toml windows_native_install_uninstall_smoke -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw "Windows native smoke test failed." }
} finally {
    Remove-Item Env:FONTCATCH_WINDOWS_SMOKE
}
```

Build a Windows x64 NSIS installer from Windows:

```powershell
npm run desktop:build -- --target x86_64-pc-windows-msvc --bundles nsis
```

The output is in `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`. Preview installers use the current-user installation mode and download WebView2 if it is missing. This command does not sign the installer.

### Separate QA app

The separate configuration `src-tauri/tauri.qa.conf.json` builds a QA application with its own settings and database. It still uses the real current-user font directory when installation is requested.

```sh
npm run desktop:build -- --debug --bundles app --config src-tauri/tauri.qa.conf.json
```

## Publish a Windows preview

Update the package, Cargo and Tauri versions together, including lockfiles. From GitHub Actions, open **CI**, choose **Run workflow** on `main`, and enable **Publish a Windows preview after all checks pass**.

Publication waits for all three validation jobs. It uses only the installer produced by that execution, includes a SHA-256 checksum, and creates a prerelease at the exact tested commit. Existing tags and releases are never replaced. Normal pushes and pull requests do not publish releases. Preview installers are unsigned; signing and manual Windows desktop validation are separate release requirements.

## Licensing

Submit only code and assets you have the right to contribute. Contributions to Fontcatch are distributed under the repository's MIT license; retain existing third-party notices. Do not add fonts from downloads or purchases as fixtures.
