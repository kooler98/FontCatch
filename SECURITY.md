# Security

Fontcatch is an early alpha. macOS Apple Silicon has been tested locally. Version 0.3.5 adds a Windows x64 preview with [passing automated Windows checks](https://github.com/kooler98/FontCatch/actions/runs/40540707057/job/122318362840). Linux remains experimental and is not ready for distribution.

Windows CI passed Rust tests, native font registration and removal, an unsigned installer build, and application startup on GitHub's Windows Server 2022 runner with administrator privileges. These checks do not establish Windows 10/11 desktop usability or operation from a standard user account. The installer wizard and visible interface still need manual validation.

Windows installation is configured for the current user, and the application manifest uses `asInvoker`. Fontcatch writes font files under the user's local application data and registration values under `HKEY_CURRENT_USER`; it does not request elevation. `asInvoker` preserves the caller's privileges, so running it from an elevated process still runs it elevated. The Windows preview has no code-signing certificate. The local macOS build is not a signed and notarized distribution release.

See the [security and performance review](docs/security-and-performance.md) for the checks performed and unresolved dependency warnings. Linux distribution is blocked by the documented GTK/glib advisory. Fontcatch does not provide an independent security audit of downloaded fonts; the operating system and webview also parse those files.

## Report a vulnerability

Do not post exploit details, private font files, download tokens or local library databases in a public issue.

If GitHub shows **Report a vulnerability** in this repository's Security tab, use that private channel. Otherwise, contact [@pierrreweb](https://twitter.com/pierrreweb) to arrange a private report before sharing sensitive details. Include the Fontcatch version, operating system, expected behavior and a minimal reproduction using an original or openly licensed test font where possible.

## Scope

Useful reports include unsafe archive extraction, unintended access outside configured folders, unconfirmed replacement or removal of unrelated fonts, unsafe external links, and excessive resource use triggered by an input file. Please distinguish application behavior from permissions already granted to another process running as the same OS user.

Dependencies and platform support will be reviewed before a supported binary release. No response-time guarantee is offered at this stage.
