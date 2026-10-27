# Security and performance review — 0.3.5 Windows preview

Reviewed on 27 October 2026 using local macOS checks and Windows CI. This is a development review, not an independent security audit or a guarantee of safety for arbitrary fonts.

## Changes

- Removed decorative captions, empty-state ornaments and unconfigured external-link buttons. The footer contains only the creator credit. Loading uses a CSS spinner without a React timer.
- Display recent history in batches of 50. Collapsed rows do not mount their details; SQLite selects at most 500 recent jobs plus all pending jobs. Old jobs remain in the database. Installed fonts retain their license metadata; completed history no longer duplicates it.
- Use native filesystem events for detection, with a 30-second fallback scan at idle. Queue overflow and directory changes trigger a coalesced rescan. Canonicalize watched roots, reject hidden paths and links below those roots, and keep filesystem scanning off the UI thread.
- Bound event and command queues, archive entries, expanded sizes, font faces, metadata strings and file reads. Open regular files without following a replacement final symlink; reject non-regular files and Windows reparse points. Recheck hashes after copying.
- Stage ZIP font entries under generated flat filenames. Preserve traversal, symlink, compressed and expanded size checks. No archive permissions or executable files are applied.
- Stream each ZIP into a private temporary snapshot before parsing. Preflight the raw central directory before the ZIP library allocates entry metadata: at most 10,000 records and 16 MiB of directory metadata. Reject duplicate names, inconsistent counts and malformed records; parsing uses the same stable snapshot, which is deleted afterward. ZIP64, split archives, trailing data and ambiguous end records are rejected. Standard single-disk ZIP32 archives are supported within the size limits.
- Make the application data and staging directories owner-only on Unix. Strip credentials, query strings and fragments from source URLs, including existing database records. This intentionally keeps only the source host and path.
- Limit Tauri commands to an explicit local-panel capability. Deny remote page navigation and new webview windows; restrict external links to validated HTTP(S) URLs. Production developer tools are disabled. CSP forbids frames, embedded objects, base URL changes and form submissions.
- Replace the unmaintained font metadata parser with Fontations `read-fonts`. Validate font table bounds, unique table tags, the header and required metadata before installation or preview. TTC preview extraction remains in memory and never rewrites the installed collection.
- Update `zip` to 9.1.2 and retain only the required Deflate backend. Enable release size optimization and thin LTO.
- On Windows, use the current user's font directory and `HKEY_CURRENT_USER`. Registry value names include the owned filename, so a new version can be registered without overwriting the old version or another application's mapping. Check every destination value before loading the font; remove only values whose type and UTF-16 path match the owned file. Preserve legacy mappings that point elsewhere. On a registry-write failure, attempt to remove values written by that attempt and release its font resource; the installation journal supports cleanup if rollback is incomplete.
- Preserve Windows paths as UTF-16 rather than converting them through lossy UTF-8. Reject embedded NULs, paths outside the configured font directory, and filenames that do not match Fontcatch's generated format. Track registrations within the process to avoid repeated resource additions; bound native unregistration attempts. Files that remain locked are handled by the existing uninstall error and rollback path.
- Deliver Windows font-change notifications asynchronously. Embed a Common-Controls v6 manifest in both the application and Rust test executables; this addresses the upstream [Tauri test-loader issue](https://github.com/tauri-apps/tauri/issues/13419). The manifest requests `asInvoker` with `uiAccess=false`, without requesting elevation.

## Dependency audit

`npm audit`: zero reported vulnerabilities. `cargo audit`: zero entries in the vulnerability list after the ZIP upgrade, using the RustSec database fetched on 27 October 2026.

Remaining RustSec **warnings are not hidden or ignored**:

| Package | Advisory | Status |
| --- | --- | --- |
| glib 0.18.5 | [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) | Unsound iterator implementation in the GTK dependency tree used by Tauri on Linux. Requires an upstream-compatible GTK/glib update before Linux release. |
| proc-macro-error 1.0.4 | [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html) | Unmaintained transitive dependency. |
| zbus_names 4.3.5 | [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html) | Unmaintained transitive dependency. |
| zvariant 5.15.1 | [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html) | Unmaintained transitive dependency. |
| zvariant_derive 5.15.1 | [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html) | Unmaintained transitive dependency. |

These packages are absent from the `aarch64-apple-darwin` dependency tree. The lockfile includes dependencies for all supported platforms. A successful `cargo audit` exit alone does not mean that every warning is resolved.

## Previous macOS verification — 0.3.4

- 83 Rust tests: archive traversal and expansion limits, raw ZIP metadata limits and stable snapshots, malformed fonts, symlinks, duplicate/update handling, recovery after interrupted operations, guarded uninstall, URL migration, bounded history and live macOS filesystem events.
- 8 TypeScript tests and 8 browser interaction tests, including real FontFace loading from synthetic fixtures, per-style previews, license text rendering, confirmation/cancellation, stale uninstall plans and a 130-entry history.
- Clippy on all targets available to the macOS build with warnings treated as errors; TypeScript and production frontend build.
- Native macOS install/register/preview/trash smoke test using an original CC0 synthetic font. The fixture source is preserved.
- Earlier packaged builds were reopened on macOS. Native settings and history loaded with the restricted capability, and a real installed font rendered in the preview. The existing managed font files and their hashes were unchanged after the schema migration; the data directory permissions were verified as `0700`.

The 0.3.4 Apple Silicon build contains 16,594,347 bytes (15.83 MiB); its verified ZIP is 4,466,764 bytes, compared with 5,217,712 bytes for 0.3.0. This is a package-size measurement, not a claim of equivalent RAM savings. Full fallback directory scans are 15 times less frequent at idle; event-driven detection is retained.

## Verification — 0.3.5

The frontend build, 3 native-launcher tests, 8 TypeScript tests, 8 browser interaction tests, and `npm audit` passed for 0.3.5.

The same CI run passed macOS formatting, Clippy and 83 Rust tests. The Apple Silicon application bundle was also built locally; its interface was not retested manually for 0.3.5.

The [Windows x64 CI job](https://github.com/kooler98/FontCatch/actions/runs/40540707057/job/122318362840) passed all of the following checks:

- Clippy for all Windows targets with warnings treated as errors, and **84 Rust tests passed** on `x86_64-pc-windows-msvc`. The native smoke test was excluded from the ordinary suite and then run separately.
- **1 native smoke test passed**: installed the original CC0 synthetic fixture, verified the actual font family selected by GDI, verified registry values, repeated registration, unregistered the font, moved its file to the Recycle Bin, and confirmed that the source fixture was unchanged. The test refuses to proceed if the test font or destination already exists.
- Built an unsigned **2.66 MiB NSIS installer**, configured for the current user.
- Started the release application executable, verified that it remained running for ten seconds and initialized its library database. This check did not run the NSIS installer or exercise the visible interface.

The runner is **Windows Server 2022 with administrator privileges**. These checks do not validate a Windows 10/11 interactive desktop session, a standard user account, the installer/uninstaller wizard, tray positioning, application previews in WebView2, reboot persistence, or fonts held open by other applications. Those remain separate validation tasks. The application is configured to avoid requesting elevation, but `asInvoker` inherits its caller's token and does not turn an administrator CI process into a standard user process.

Windows preview packages have **no code-signing certificate**. Building an installer, passing tests, and applying a Windows manifest do not provide publisher verification or an independent security audit. The local Apple Silicon build is not a signed/notarized distribution release. Linux distribution remains blocked by the GTK/glib advisory above.

Fonts are also parsed by the OS and system webview; keeping those components updated remains necessary.
