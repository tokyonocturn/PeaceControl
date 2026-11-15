# Building PeaceControl

## Prerequisites

| Tool | Version | Purpose |
| --- | --- | --- |
| Rust (stable, MSVC toolchain) | 1.80 or newer | Everything |
| Visual Studio Build Tools | 2019 or newer, "Desktop development with C++" | Linker for the MSVC target |
| NSIS | 3.08 or newer | Building `PeaceControl-Setup-*.exe` (optional) |

Install Rust from <https://rustup.rs> and confirm the host target:

```powershell
rustc -vV     # host: x86_64-pc-windows-msvc
cargo --version
```

The project targets Windows only. Cross compilation to Windows from another
operating system is not supported because the application uses the Windows
registry and native file dialogs.

## Build and test

```powershell
cargo test --workspace        # 133 tests
cargo build                   # debug binaries in target\debug
cargo build --release         # release binaries in target\release
```

Release binaries:

```text
target\release\peacecontrol.exe       GUI, self contained
target\release\peacecontrol-cli.exe   command line companion
```

The GUI binary has no runtime dependency: it is a single executable that links
the C runtime statically through the MSVC toolchain defaults used by Rust.

## Run

```powershell
.\target\release\peacecontrol.exe
.\target\release\peacecontrol-cli.exe doctor
```

## Portable build

Portable mode is a marker file, not a separate build:

```powershell
New-Item -ItemType Directory -Force dist\portable | Out-Null
Copy-Item target\release\peacecontrol.exe dist\portable\PeaceControl.exe
Copy-Item target\release\peacecontrol-cli.exe dist\portable\
New-Item -ItemType File dist\portable\peacecontrol.portable
Copy-Item README.md, LICENSE dist\portable\
Compress-Archive -Path dist\portable\* -DestinationPath dist\PeaceControl-1.1.0-portable.zip -Force
```

With `peacecontrol.portable` beside the executable, settings, presets, logs and
snapshots are stored in `data\` in that folder. `PEACECONTROL_PORTABLE=1` and
`PEACECONTROL_DATA_DIR` override the location for testing.

## Installer

The installer is an NSIS script that copies the release binary, writes Start
menu entries, registers an uninstaller in *Apps & features* and removes
everything again on uninstall.

```powershell
& "C:\Program Files (x86)\NSIS\makensis.exe" /DAPP_VERSION=1.0.0 installer\PeaceControl.nsi
```

Output: `dist\PeaceControl-Setup-1.1.0.exe`.

The uninstaller (`uninstall.exe`, written into the install folder) removes
program files, shortcuts and registry entries. It asks before deleting
`%APPDATA%\PeaceControl`, so user configurations survive a reinstall by default.
A silent uninstall keeps user data.

## Windows integration

The tray icon, the global hotkeys and the foreground-application watcher use
Win32 directly (`Shell_NotifyIconW`, `RegisterHotKey`, `GetForegroundWindow`)
through the `windows-sys` bindings. They need an interactive desktop session:
over a service or a session without an explorer shell, the application still
runs and edits configurations, but reports in Settings that the tray and
hotkeys are unavailable instead of pretending they work.

Global hotkeys are registered only while PeaceControl runs and are released on
exit. A combination already owned by another program is reported in Settings.

## Everything at once

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 -Version 1.0.0
```

This runs the tests, builds release binaries, creates the portable zip and, when
NSIS is installed, the setup executable. Output lands in `dist\`.

## Verifying a build

```powershell
cargo test --workspace
.\target\release\peacecontrol-cli.exe doctor          # shows detected Equalizer APO
.\target\release\peacecontrol-cli.exe presets         # built in library
.\target\release\peacecontrol.exe                     # interface
```

`doctor` is the fastest way to confirm the Equalizer APO integration: it prints
the detected installation folder, every configuration target and the data
folder the application will use.

## Troubleshooting

**`error: linker link.exe not found`** — install the Visual Studio C++ build
tools and start a new shell.

**The interface reports "Equalizer APO not detected"** — install Equalizer APO
or open Settings and select its folder. Detection checks for a `config`
subfolder or `EqualizerAPO.dll` in the usual Program Files locations.

**Apply does nothing** — the selected target must be inside the Equalizer APO
`config` folder. Program Files is protected; run PeaceControl as administrator
if Equalizer APO was installed for all users and the folder is not writable.

**A build fails after dependency updates** — the workspace pins major versions
in `Cargo.toml` and commits `Cargo.lock`. Run `cargo update -p <crate>` only for
the crate you mean to move.
