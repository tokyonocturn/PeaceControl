# Contributing to PeaceControl

Thanks for helping. PeaceControl is a native Rust front end for Equalizer APO:
it writes configuration files and never processes audio. Keep that boundary in
mind — a feature that needs to touch the audio stream does not belong here.

## Setting up

You need Windows 10 or 11, the Rust MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`)
and the Visual Studio C++ build tools. NSIS 3 is only needed to build the
installer.

```powershell
cargo build
cargo run -p peace-control-app
```

The application stores its data in `%APPDATA%\PeaceControl`. To keep an
experiment away from your real settings, create a file called
`peacecontrol.portable` next to `peacecontrol.exe`; the application then uses
`data\` beside the executable instead.

## Checks before pushing

CI runs exactly these three commands on Windows and so should you:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Tests must not need a network connection, a desktop session or a real
Equalizer APO installation. Windows-only behaviour is tested through the
`cfg(windows)` guards already in the tree (for example the tray self test in
`crates/peace-control-app/src/tray.rs`), and must skip cleanly where no desktop
exists rather than fail.

## Layout

| Path | Contents |
| --- | --- |
| `crates/peace-control-core` | model, DSP, formats, persistence. No UI, no unsafe. |
| `crates/peace-control-app` | egui interface, tray, hotkeys, file dialogs. |
| `crates/peace-control-cli` | scriptable companion. |
| `installer/` | NSIS script for the installer and uninstaller. |
| `scripts/build-release.ps1` | tests, release build, portable zip, installer. |
| `docs/` | audit of the original application, architecture, compatibility, build notes. |

Pure logic belongs in the core crate so it can be tested without a window.
Platform calls belong in `platform.rs` or `tray.rs`, behind `cfg` guards and
with a non-Windows fallback so the crate still compiles elsewhere.

## Code style

- Comments explain why, not what. Avoid documenting obvious code.
- No `unsafe` in the core crate (it is `#![forbid(unsafe_code)]`); the Win32
  layer in the app crate is the only place that needs it, and every block is
  small and commented by the API it calls.
- Errors are typed (`peace_control_core::Error`) and reported; do not swallow a
  failure with `let _ =` unless the operation is genuinely optional (logging,
  clipboard, tray refresh).
- New settings need a default, sanitiser coverage and a line in
  `docs/COMPATIBILITY.md` if they mirror an original Peace feature.

## Commits

Write one commit per coherent change that leaves the tree building. A commit
message says why the change exists; the diff already says what changed. Include
tests in the same commit as the behaviour they cover. Never commit empty
changes, generated binaries, `analysis/` or `dist/`.

## Releasing

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Version 1.2.1
```

That runs the tests, builds optimised binaries, produces
`dist\PeaceControl-1.2.1-portable.zip` and, when NSIS is installed,
`dist\PeaceControl-Setup-1.2.1.exe`. Then:

1. Update `Cargo.toml`, `CHANGELOG.md`, the README download names and the audit
   or compatibility docs for anything the release changes.
2. Tag the commit (`git tag -a v1.2.1`) and push it with the branch.
3. Attach both artefacts to the GitHub release.
4. Verify by cloning the tag into a clean directory and running
   `cargo build --release` and `cargo test` there.

## Licensing

Contributions are MIT licensed. Do not copy code, icons, themes, help text or
presets from Peace, Equalizer APO or any other product into this repository;
describe the behaviour you want and implement it here.
