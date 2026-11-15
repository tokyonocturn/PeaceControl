# Compatibility

## File formats

### `.peace` presets (interoperable)

PeaceControl reads and writes the INI dialect used by the original tool.

Supported on read:

- `[General]`: description, preamp, per bank preamps, routing, bass/treble tone,
  delay, echo, crossfeed, fake stereo, speaker swap, reverse, upmix, 5.1/7.1
  downmix, Graphic EQ flag.
- `[Speakers]` with any number of banks and arbitrary channel target strings.
- `[Frequencies]`, `[Qualities]`, `[Gains]`, `[Filters]`, `[Disabled]` for every
  bank, including banks whose sections are missing (they inherit the shared
  sections, which is what the original writes for identical banks).
- `[Commands]` values containing several tab separated commands.
- `[Configuration] HotKey`, `[Surround]` levels/delays/widths, `[Sliders]`
  labels.

Written on export: the same sections, CRLF line endings, band values for every
band so other readers never have to infer defaults.

Filter code mapping:

| PeaceControl shape | `.peace` code | Equalizer APO token |
| --- | --- | --- |
| Peak | 0 | `PK` |
| Low pass | 1 | `LP` |
| High pass | 2 | `HP` |
| Band pass | 3 | `BP` |
| Low shelf | 4 | `LS` |
| High shelf | 5 | `HS` |
| Notch | 6 | `NOTCH` |
| All pass | 7 | `AP` |
| Low shelf 12 dB | 8 | `LS 12dB` |
| High shelf 12 dB | 9 | `HS 12dB` |
| Low pass with Q | 10 | `LPQ` |
| High pass with Q | 11 | `HPQ` |
| Low shelf 6 dB | 14 | `LS 6dB` |
| High shelf 6 dB | 15 | `HS 6dB` |

Codes 12 and 13 appear in shipped presets for low pass variants; they are read
as low pass and written back as 1 (or 10 with an explicit Q). The original's
Linkwitz-Riley and Butterworth menu entries are combined responses, not distinct
Equalizer APO filter types; use low/high pass with the appropriate Q instead.

### Equalizer APO configurations (import)

Parsed constructs: `Preamp:`, `Channel:`, `Filter:` (with optional numeric index,
`ON`/`OFF`, `Fc`, `Gain`, `Q` and two token shelf names such as `HS 12dB`),
`GraphicEQ:` points, and every other line preserved verbatim as a raw command.
`Include:` and `Device:` lines are kept as commands and reported in the import
notes, because inlining another file would change which device the
configuration applies to.

`GraphicEQ:` curves are converted to editable peak bands on the measurement's
own frequency grid, with Q derived from the grid spacing. The resulting curve is
visually equivalent but not bit identical to the original convolution.

### Measurement import

- AutoEQ `ParametricEQ.txt` → filters.
- AutoEQ `GraphicEQ` lines and `frequency,gain` CSV tables → bands.
- Room EQ Wizard filter export (`* Filter 1: ON PK Fc ...`) → filters.
- Semicolon, comma, tab and space separated numbers are all accepted; decimal
  commas are normalised.

### Export

- Equalizer APO configuration text (byte identical to what `Apply` writes).
- Audacity equalizer XML (`<equalizationeffect>`) and TXT (`frequency<TAB>gain`),
  sampled at 240 points from 10 Hz to 20 kHz.

## Feature matrix

| Capability | Original Peace 1.6.9 | PeaceControl 1.0 |
| --- | --- | --- |
| Parametric EQ, 31 bands | Yes | Yes |
| Speaker banks and channel targets | Yes | Yes |
| Preamp, global and per bank | Yes | Yes |
| Response graph, drag editing | Yes | Yes |
| Preamp in graph | Yes | Yes |
| REW SPL overlay / image overlay | Yes | No |
| Filter shapes (peak, shelves, pass, notch, all pass) | Yes | Yes |
| Linkwitz-Riley / Butterworth menu entries | Yes | Use LP/HP with Q |
| Bass/treble shelves | Yes | Yes |
| Crossfeed, duophonic, echo, delay | Yes | Yes (command approximations) |
| Channel swap/reverse, upmix, downmix | Yes | Yes |
| Surround effect | Yes | Yes (width/level/delay stage) |
| Graphic EQ (convolution) | Yes | Yes |
| Raw commands editor | Yes | Yes |
| VST plugin configuration editor bridge | Yes | No (paste `Plugin:` lines) |
| Preset library | Yes | Yes (own, nine presets) |
| Preset import/export `.peace` | Yes | Yes |
| Audacity export | Yes | Yes |
| Device-specific config selection | Yes | Yes |
| Automatic backups | Yes | Yes |
| Hotkeys (in application) | Yes | Yes |
| Global hotkeys | Yes | Yes, three configurable bindings registered with Windows |
| Tray menu | Yes | Yes: show/hide, enable, configurations, settings, exit, all dispatched through `WM_COMMAND` |
| Process automation rules | Yes | Yes, foreground process matched against user rules |
| Device attachment automation | Yes | No, choose a configuration target per device manually |
| Peak value meter | Yes | No: PeaceControl never touches the audio stream, so it has no samples to meter; Equalizer APO's own readout is unaffected |
| Update check / website integrations | Yes | No (offline by design) |
| Languages | 10+ | English |
| Themes/skins | Yes | Dark/light + accent colour |
| Settings persistence | `Peace.ini` | `settings.json` |
| Crash recovery | No | Yes |
| Command line tool | Limited | Yes |
| Portable mode | No | Yes |

## Known behavioural differences

1. **Preset storage.** PeaceControl keeps its library as JSON under
   `%APPDATA%\PeaceControl\presets`; `.peace` files are an exchange format. Use
   **Export .peace…** to produce a file the original tool can open.
2. **Effects are rendered as standard commands.** Crossfeed, duophonic widening
   and surround are built from `Copy:`, `Delay:` and `Filter:` lines. They are
   valid Equalizer APO commands and produce the intended effect, but the exact
   command list differs from the original.
3. **Graphic EQ conversion.** Imported convolution curves become parametric
   bands; the curve shape is preserved at the sampled points, not sample-exact.
4. **Live apply.** When "Apply automatically" is on, the target file is rewritten
   shortly after an edit completes. Equalizer APO reloads it; this can produce a
   short audio interruption, exactly as editing the file by hand does.
5. **Tray and hotkeys.** The tray icon exists while the application runs and
   disappears when it exits; there is no "always resident" background service.
   Menu selections arrive as `WM_COMMAND`, the message Explorer sends, so a menu
   click and a posted command run the same handler. While the application runs in
   the tray its window is parked off-screen without a taskbar button rather than
   hidden: Windows does not paint a hidden window, and the interface only drains
   tray and hotkey events while it paints. Show puts the window back at the
   position it was parked from. Hotkeys are registered with `RegisterHotKey`, so a
   combination another program already owns is reported in Settings instead of
   silently doing nothing. Leaving a hotkey field empty disables that binding.
6. **Enabling and bypassing.** Switching the equalizer off writes an empty
   configuration file, which is how Equalizer APO is told to stop applying
   filters. The previous file is kept as `.bak` by the atomic writer, and turning
   the equalizer back on renders the current configuration again.
7. **Automation scope.** Only the process that owns the foreground window is
   matched, by name and case-insensitively with an optional `.exe`. Rules are
   evaluated in order and the first match wins; a rule whose configuration was
   deleted is ignored. Nothing is matched while PeaceControl itself is in front,
   and when no rule matches the current configuration is left alone.
8. **Automation and unsaved work.** Switching configurations through the tray, a
   hotkey or a rule saves the configuration being edited first, so a switch never
   discards changes.
