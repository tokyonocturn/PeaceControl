# PeaceControl

**Equalizer APO, under control.** PeaceControl is a native Windows desktop
application for building Equalizer APO configurations: a parametric equalizer
with a live response graph, per channel speaker banks, effects, presets,
measurement import and one click applying.

It is an independent, MIT licensed reimplementation of the feature set of the
Peace configuration tool, written from scratch in Rust. PeaceControl is not
affiliated with Peace, Equalizer APO or any other equalizer product, and it
contains no code or assets from them.

- Runs on **Windows 10 and Windows 11** (64-bit).
- **Native**: one self-contained executable, no runtime, no installer required,
  no background service, no telemetry, no account, no paywall.
- **Editable**: everything is plain files. Presets are JSON, exports are
  Equalizer APO command text, and the interoperable `.peace` dialect is read
  and written so existing configurations keep working.

## Features

| Area | What you get |
| --- | --- |
| Equalizer | 1–31 bands per speaker bank, peak / shelf / pass / notch / all-pass shapes, per band frequency, gain and Q |
| Response graph | Predicted response from the actual filters, draggable handles, hover read-out, preamp overlay |
| Speakers | Any number of speaker banks, each aimed at any Equalizer APO channel list (`all`, `L R`, `L C R`, `SUB`, `SL SR`, `RL RR`, custom targets) |
| Preamp | Global and per bank, with clipping awareness |
| Effects | Bass and treble shelves, crossfeed, duophonic widening, echo, left/right swap, delays, 5.1 and 7.1 downmix, upmix, convolution (Graphic EQ), routing expressions |
| Surround | Width, level and delay based stereo spread written as ordinary commands |
| Presets | Original built in library, searchable list, duplicate, delete, `.peace` import/export |
| Import | Equalizer APO `config.txt`, Room EQ Wizard filter exports, AutoEQ parametric and GraphicEQ files, `frequency,gain` CSV tables |
| Export | Equalizer APO configuration, Audacity XML and TXT equalizer curves |
| Commands | Raw Equalizer APO editor with a live rendered preview |
| Files | Detects the Equalizer APO installation, lists every configuration target, writes atomically with a `.bak` |
| Safety | Snapshots of the whole configuration folder, crash recovery drafts, rotating log, clear error messages |
| Interface | Original branding, dark and light themes, accent colour, keyboard shortcuts, no proprietary assets |

## Install

### Installer

Download `PeaceControl-Setup-1.0.1.exe` and run it. The installer adds Start
menu shortcuts and an uninstaller under *Apps & features*. It requires
administrator rights because it installs into `Program Files`.

The uninstaller removes the application. Your configurations, settings and
snapshots in `%APPDATA%\PeaceControl` are kept unless you explicitly ask for
them to be removed.

### Portable

Download `PeaceControl-1.0.1-portable.zip`, unpack it anywhere and run
`PeaceControl.exe`. The folder contains a `peacecontrol.portable` marker, so
settings, presets and logs are stored in `data\` next to the executable. Move
the folder to move the whole installation, including onto a USB stick.

### Requirements

- Windows 10 or Windows 11, 64-bit.
- [Equalizer APO](https://sourceforge.net/projects/equalizerapo/) installed and
  bound to an audio device. PeaceControl edits its configuration; it does not
  process audio itself.

## Quick start

1. Start PeaceControl. The header shows whether Equalizer APO was detected.
   If it was not, open **Settings** and choose the installation folder.
2. Pick a configuration on the left, or press **+ New**.
3. Drag the handles in the graph or the sliders below it. Hold **Shift** while
   dragging a graph handle to change its frequency.
4. Press **Apply** (or `Ctrl+E`) to write the configuration Equalizer APO reads,
   and **Save** (`Ctrl+S`) to keep the preset.

`Apply` writes the selected target (by default `config.txt`) inside
`<EqualizerAPO>\config`. Choose another target in **Settings** to equalize only
one device.

## Keyboard shortcuts

| Keys | Action |
| --- | --- |
| `Ctrl+S` | Save the configuration |
| `Ctrl+E` | Apply to Equalizer APO |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo |
| `Ctrl+O` | Open a `.peace` or configuration file |
| `Shift` + drag | Change a band frequency on the graph |
| Right click a band or slider | Reset its gain to 0 dB |
| `F1` | About, diagnostics and shortcut reference |

## Where files live

| Path | Contents |
| --- | --- |
| `%APPDATA%\PeaceControl\settings.json` | Interface and integration settings |
| `%APPDATA%\PeaceControl\presets\*.json` | Your configurations |
| `%APPDATA%\PeaceControl\backups\` | Snapshots of the Equalizer APO config folder |
| `%APPDATA%\PeaceControl\recovery\draft.json` | Unsaved work from the last session |
| `%APPDATA%\PeaceControl\logs\peacecontrol.log` | Rotating log |

In portable mode the same layout lives in `data\` beside the executable.

## Command line companion

Every engine feature is scriptable:

```text
peacecontrol-cli render "Studio Flat 13" --output config.txt
peacecontrol-cli import my-measurement.csv --output curve.json
peacecontrol-cli validate "C:\Users\me\AppData\Roaming\PeaceControl\presets\Bass.json"
peacecontrol-cli apply curve.json --target "C:\Program Files\EqualizerAPO\config\headset.txt"
peacecontrol-cli presets
peacecontrol-cli doctor
```

## Compatibility with Peace

PeaceControl reads and writes the `.peace` INI dialect, so presets written by
the original tool can be imported and exported again. It keeps the same
concepts: one equalizer bank per speaker group, `[Frequencies]`, `[Qualities]`,
`[Gains]`, `[Filters]`, `[Commands]`, `[Surround]` and `[Sliders]` sections, and
the same filter numbering for the shapes both applications support.

What is intentionally different:

- PeaceControl stores its own presets as readable JSON; `.peace` is the
  interchange format.
- Some original-only features are not reproduced: the system tray menu,
  process triggered automation, global hotkeys, the peak value meter, the VST
  configuration editor bridge, in-app update checks and the multilingual
  interface. Effects that depended on those are still written correctly.
- Some details are approximated from the shipped documentation and preset
  data because no decompiler was available for the original binary; see
  [docs/AUDIT.md](docs/AUDIT.md) and [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md).

## Build from source

```powershell
git clone https://github.com/tokyonocturn/PeaceControl
cd PeaceControl
cargo test            # unit and integration tests
cargo build --release # target\release\peacecontrol.exe
```

Packaging (portable zip plus NSIS installer) is described in
[docs/BUILD.md](docs/BUILD.md) and implemented by
`scripts/build-release.ps1`.

## Project structure

```text
crates/peace-control-core   domain model, format engine, persistence, tests
crates/peace-control-app    native desktop interface (egui/glow)
crates/peace-control-cli    scriptable companion
installer/                  NSIS installer and uninstaller
docs/                       audit, architecture, compatibility, build notes
```

## License

MIT. See [LICENSE](LICENSE).

PeaceControl is an independent project. Equalizer APO is a separate open source
project by Jonas Thedering and contributors; Peace is a separate application by
P.E. Verbeek. Neither is bundled with PeaceControl, and no assets from either
are redistributed here.
