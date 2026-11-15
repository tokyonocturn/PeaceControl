# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.1] - 2026-11-15

No functional change: the application behaves exactly as 1.2.0. This release
exists so the shipped tag has a green test run.

### Fixed

- The command line tests shared one temporary fixture path, so tests running in
  parallel could read a file another had just truncated and fail. Every fixture
  now gets its own file.

## [1.2.0] - 2026-11-15

Makes the tray menu work through the same path Windows uses, keeps it available
while the application runs in the tray, and proves it end to end.

### Added

- The tray menu is built from data (`menu_tree`) with stable command identifiers,
  so labels, check marks and identifiers can be tested without a desktop.
- `Shell::post_command` and an in-tree test that posts real `WM_COMMAND`
  messages and asserts the resulting events.
- `scripts/verify-tray.ps1`: an end-to-end check that drives a running build by
  posting the `WM_COMMAND` messages the menu items carry, then asserts window
  position and extended style, `settings.json`, the applied configuration, the
  log and a restart.

### Fixed

- **Tray and hotkey commands could not be reached at all.** The popup menu used
  `TPM_RETURNCMD` and handled the result inline, so no `WM_COMMAND` was ever
  produced; posted commands were ignored and a menu selection ran a second,
  different code path. Selections now arrive as `WM_COMMAND`, exactly like
  Explorer sends them, and every command is decoded in one place.
- **Running in the tray stopped all shell integration.** Hiding the window also
  stopped egui from calling `update`, which is what drains the tray and hotkey
  queue, so Show did nothing and hotkeys went dead. Close to tray, minimize to
  tray and the tray button now park the window off-screen without a taskbar
  button, which keeps the queue draining; Show restores the saved position.
- **The idle interface never repainted**, so tray, hotkey and automation polling
  could wait indefinitely for pointer movement. The interface requests a repaint
  every 250 ms.
- Settings opened from the tray is now recorded in the log, and the tooltip,
  check marks and status line follow the state the command actually set.

## [1.1.0] - 2026-11-15

Closes the last feature gaps against the original application: tray, global
hotkeys and per-application automation are now real, not documented gaps.

### Added

- **Tray icon** created with `Shell_NotifyIconW`, with a native menu: show or
  hide the window, enable or bypass the equalizer, switch configuration, open
  settings and exit. Settings gains "Close to tray" and "Minimize to tray", and
  a tray button in the header hides the window on demand.
- **Global hotkeys** registered with `RegisterHotKey` while the application
  runs: toggle the equalizer and step to the previous or next configuration.
  All three are editable in Settings, validated as they are typed, re-registered
  immediately when changed and released on exit. A combination owned by another
  program is reported instead of failing silently.
- **Per-application automation**: a rule maps a process name to a configuration.
  The foreground process is sampled while automation is enabled, rules are
  matched case-insensitively with an optional `.exe`, first match wins, and a
  switch saves pending edits before activating the mapped configuration. The
  Applications window lists, adds, edits and removes rules and can capture the
  application currently in front.
- **Global enable state** that actually bypasses the equalizer by writing an
  empty target configuration, restorable from the header, the tray, the menu or
  a hotkey.

### Tests

- 152 tests. New coverage for process normalisation and rule matching, the
  automation change tracker, hotkey parsing and formatting, the settings
  sanitiser for rules and hotkeys, the bypass writer, and a real global hotkey
  registration and release on Windows.

## [1.0.1] - 2026-11-15

Closes the gaps found when checking the interface against the feature list.

### Fixed

- **Solo** now actually solos the selected band: every other band of the bank is
  disabled, and pressing the same button restores them. Before, it only marked
  the configuration as modified.
- **Reverse channel order** is reachable from the Effects window. The renderer
  already wrote the command, but no control set the flag.
- **Echo repeats** can be set from 1 to 4, and the delay row is labelled
  correctly instead of "Echoes".
- The **surround stage** exposes front, centre, side, rear and subwoofer levels,
  front and subwoofer delays and the bass crossover. Only widths, rear level and
  subwoofer level were editable before.
- The **global preamp** and the **preset description** are editable from the new
  preset card in the inspector. Both were rendered and saved already, but the
  interface offered no way to change them.
- **Confirm before closing** works: closing the window with unsaved work asks
  whether to save, close without saving or keep editing.
- **Log level** is a setting that takes effect immediately, rather than a value
  that was only read at startup.
- Removed the unused `show_tooltips` and `language` settings, so the settings
  file contains only options the application honours.

### Tests

- 135 tests, three of them new: solo toggling and log level parsing.

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
