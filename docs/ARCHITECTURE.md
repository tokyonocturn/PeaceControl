# Architecture

PeaceControl is a three crate Cargo workspace. The split exists so the
configuration engine can be tested without a window and reused by the CLI.

```text
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ peace-control-app            │   │ peace-control-cli            │
│ native window, graph, panels │   │ render/import/apply commands │
└───────────────┬──────────────┘   └──────────────┬───────────────┘
                │                                 │
                └────────────────┬────────────────┘
                                 ▼
                 ┌────────────────────────────────┐
                 │ peace-control-core             │
                 │ model · dsp · formats · files  │
                 └────────────────────────────────┘
```

## Crates

### `peace-control-core`

No UI dependencies, no unsafe code (`#![forbid(unsafe_code)]`).

| Module | Responsibility |
| --- | --- |
| `model` | `Preset`, `SpeakerEq`, `Band`, `FilterType`, `Effects`, `Surround`; ranges and clamping |
| `dsp` | RBJ biquad coefficients and predicted magnitude response, graphic EQ sampling |
| `render` | preset → Equalizer APO command text |
| `parse` | Equalizer APO text → preset (channel blocks, filters, GraphicEQ, passthrough) |
| `peace_ini` | interoperable `.peace` reader/writer |
| `autoeq` | AutoEQ, REW and CSV measurement import |
| `audacity` | Audacity XML and TXT curve export |
| `library` | nine original built-in presets |
| `preset_store` | JSON preset files in one folder |
| `settings` | application settings with sanitising and defaults |
| `paths` | Equalizer APO discovery, config targets, app data, portable mode |
| `backup` | folder snapshots, pruning, restore |
| `session` | crash recovery drafts |
| `logging` | rotating file logger |
| `fsutil` | atomic writes, `.bak` copies, safe reads |

### `peace-control-app`

Native window through `eframe`/`egui` with the `glow` renderer: pure Rust, no
web view, no Electron, no external runtime. Modules: `theme` (design tokens and
fonts), `brand` (procedural logo and window icon), `graph` (response drawing and
interaction), `widgets` (band strip and small controls), `engine` (Equalizer
APO integration), `platform` (autostart, Explorer, theme preference), `app`
(state and layout) and `dialogs` (effects, settings, commands, import, backups,
help, recovery).

### `peace-control-cli`

A thin command layer over the core: `render`, `validate`, `import`, `apply`,
`presets`, `library`, `export-audacity`, `doctor`. Command handling returns its
output as a string so it can be unit tested without spawning processes.

## Data flow

```text
edit ─▶ Preset (in memory) ─▶ render_config ─▶ Equalizer APO config.txt
             │                                        │
             │                                        ├─ .bak of previous file
             └─▶ JSON preset / .peace export          └─ snapshot on exit
             └─▶ recovery draft                        └─ log entry
```

Nothing in the application touches the audio path. Equalizer APO watches its
configuration folder and reloads the file; PeaceControl only writes valid text.

## Reliability design

**Atomic writes.** `fsutil::write_atomic` writes to a temporary file in the same
directory, `sync_all`s it, then renames over the destination. A crash can leave
either the old or the new file, never a truncated one. The previous contents are
copied to `<name>.bak` first, which gives a one step manual undo even if the
application is killed.

**Recovery drafts.** While a preset is dirty the current state is written to
`recovery/draft.json` (debounced by two seconds). On the next start, if the
draft is newer than the loaded preset and less than fourteen days old, a
recovery dialog offers it back. Saving clears the draft.

**Snapshots.** The whole Equalizer APO `config` folder is copied into
`backups/snapshot-<unix>` on exit (configurable) and on demand. Old snapshots
are pruned to a configurable count. Restore copies a snapshot back over the
folder.

**Errors.** The core returns a typed `Error` with the failing path and the
underlying I/O error. The interface turns every failure into a status message
and a log line; nothing is silently swallowed. Read failures while discovering
Equalizer APO are treated as "not found" and reported as such.

**Settings and forward compatibility.** `Settings` and `Preset` are serde
structures with `#[serde(default)]`, so an older file loads after new fields are
added and a corrupt settings file falls back to defaults with a warning. All
numeric input is clamped (`Settings::sanitized`, `Band::sanitized`).

**Logging.** `logging::init` installs a logger writing
`logs/peacecontrol.log`, rotating at 512 KiB and keeping three files. Warnings
and errors are mirrored to stderr when a console is attached. The diagnostics
window shows detected paths and the log location.

**Single instance.** Not enforced: two windows may edit different presets. The
atomic writer keeps the file consistent; the last apply wins, which matches how
Equalizer APO itself behaves.

## Testing

The workspace has 132 unit and integration tests (no network, no GUI):

- model invariants, clamping, filter code round trips;
- biquad response: peak gain at centre, −3 dB at cutoff, shelf behaviour, band
  summation;
- `.peace` parsing of a realistic fixture, round trip through write and read,
  disabled bands, decimal commas, quoted values;
- render output for preamp, channel lines, filters, GraphicEQ and effects;
- import of Equalizer APO text, REW exports, AutoEQ and CSV, plus round trip of
  rendered output back into a preset;
- Audacity export shape;
- storage, snapshots, restore, pruning, drafts, settings clamping;
- engine apply without a target, apply to a temporary file, snapshot guards;
- CLI command behaviour against temporary files;
- interface helpers: frequency/gain mapping, label formatting, status fade,
  untitled numbering, slider mapping.

GUI rendering is not unit tested; it was smoke tested by starting the binary and
confirming it creates its data folders, seeds the library and stays running.

## Deliberate non-goals

- No audio processing, no driver, no service.
- No network access at all: no update checks, no telemetry, no accounts.
- No use of the original application's DLLs, skins, presets or help content.
