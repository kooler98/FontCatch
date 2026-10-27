# Security

Fontcatch is an early alpha. The current validation target is macOS Apple Silicon. Windows and Linux support is experimental; do not treat compilation or source availability as proof that those platforms are ready for distribution.

See the [security and performance review](docs/security-and-performance.md) for the checks performed and unresolved dependency warnings. Linux distribution is blocked by the documented GTK/glib advisory. Fontcatch does not provide an independent security audit of downloaded fonts; the operating system and webview also parse those files.

## Report a vulnerability

Do not post exploit details, private font files, download tokens or local library databases in a public issue.

If GitHub shows **Report a vulnerability** in this repository's Security tab, use that private channel. Otherwise, contact [@pierrreweb](https://twitter.com/pierrreweb) to arrange a private report before sharing sensitive details. Include the Fontcatch version, operating system, expected behavior and a minimal reproduction using an original or openly licensed test font where possible.

## Scope

Useful reports include unsafe archive extraction, unintended access outside configured folders, unconfirmed replacement or removal of unrelated fonts, unsafe external links, and excessive resource use triggered by an input file. Please distinguish application behavior from permissions already granted to another process running as the same OS user.

Dependencies and platform support will be reviewed before a supported binary release. No response-time guarantee is offered at this stage.
