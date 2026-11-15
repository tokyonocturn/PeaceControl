# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-15

First public release: an independent, MIT licensed replacement for the Peace
Equalizer APO configuration tool, written in Rust.

### Added

- Parametric equalizer with 1 to 31 bands per speaker bank, 14 filter shapes and
  per band frequency, gain and Q.
- Speaker bank management with arbitrary Equalizer APO channel targets and
  global and per bank preamps.
- Predicted response graph computed from the actual biquad coefficients, with
  draggable gain handles, Shift-drag frequency editing and a preamp overlay.
- Vertical band slider strip with click, drag, double click reset and selection
  shared with the graph.
- Effects: bass and treble shelves, crossfeed, duophonic widening, echo, delay,
  channel swap, upmix, 5.1 and 7.1 downmix, convolution (Graphic EQ) and routing
  expressions.
- Surround stage with width, level and delay controls.
- Preset library with nine original built in configurations, search, duplicate,
  delete and "save as copy".
- Interoperable `.peace` import and export, and JSON preset storage.
- Import of Equalizer APO configurations, Room EQ Wizard filter exports, AutoEQ
  parametric and GraphicEQ files, and `frequency,gain` CSV tables.
- Export of Equalizer APO configuration text and Audacity XML/TXT curves.
- Raw command editor with a live rendered preview.
- Equalizer APO detection, configuration target selection and atomic writes with
  a `.bak` of the previous file.
- Snapshots of the whole Equalizer APO configuration folder, with restore and
  pruning.
- Crash recovery drafts and a recovery dialog on the next start.
- Rotating file log, diagnostics window and typed error reporting.
- Original interface with dark and light themes, accent colour, procedural
  branding, keyboard shortcuts and native file dialogs.
- Command line companion: `render`, `validate`, `import`, `apply`, `presets`,
  `library`, `export-audacity` and `doctor`.
- NSIS installer with Start menu and desktop shortcuts, and an uninstaller that
  keeps user configurations unless asked otherwise.
- Portable mode driven by a `peacecontrol.portable` marker file.
- 132 tests across the engine, interface helpers and command line.

### Known limitations

- No tray menu, global hotkeys, process triggered automation or peak value
  meter; see `docs/COMPATIBILITY.md`.
- Effects that the original produced with internal generators are written as
  equivalent Equalizer APO command sequences.
- Imported convolution curves are converted to parametric bands.
- English interface only.
