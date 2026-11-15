# Audit of `PeaceSetup.exe`

This document records what was observed in the original installer, how it was
observed, and what PeaceControl reproduces. It separates **observations** (read
directly from the shipped artifact) from **inferences** (conclusions drawn from
documentation and data files) and **unknowns**.

## 1. Artifact identity

| Property | Value |
| --- | --- |
| File | `PeaceSetup.exe` |
| SHA-256 | `d007a5efcbe361efbe5ec88eea8eff6191d0188e8c7111354cbb063fbac26f2c` |
| Size | 55,657,784 bytes |
| Format | PE32 executable, x86, Windows GUI subsystem |
| Version resource | 1.6.9.11 |
| File description | "Setup Tool for Peace (Peter's Equalizer APO Configuration Extension)" |
| Product name | Peace |
| Copyright | P.E. Verbeek |

The sample was provided by the project owner. It is not redistributed with
PeaceControl and is excluded from version control (`analysis/`).

## 2. Method and evidence

Analysis used the REA (Reverse Engineer Anything) toolchain:

| Step | Tool | Result |
| --- | --- | --- |
| Manifest inspection | `rea inspect-managed-artifact` | `status: not-managed`, no CLI data directory: native Win32, not .NET |
| Resource inventory | `rea inspect-pe-resources` | PE32, 84 resource ids, 54.8 MB `.rsrc`, icon groups, manifest, version info |
| Archive/container inspection | `7z l`, byte carving | Installer is a native stub whose payload is stored in the resource section |
| Payload extraction | `7z x` + PE header parsing | Three embedded PEs recovered |
| Behavioural specification | Compiled HTML help (`ITSF`/CHM) extracted from resources | 79 topic pages describing every feature and dialog |
| Data model | 89 `.peace` presets extracted from bundled ZIP resources | Complete INI schema and value ranges |
| String survey | ASCII/UTF-8 string extraction | Translation tables, preset names, command templates, file names |

**Limitation.** The Ghidra provider was not configured on the analysis host and
Hopper does not run on Windows, so no decompilation and no native function
recovery was performed. Everything below is derived from the shipped
documentation, the shipped preset data, resource contents and the PE layout —
not from disassembled code. PeaceControl therefore reimplements *behaviour*, not
original source.

## 3. Installer composition

The outer `PeaceSetup.exe` is a native installer that embeds its payload in PE
resources. The payload contains:

| Embedded artifact | Size | Role |
| --- | --- | --- |
| `Peace.exe` | 25,324,856 bytes, PE32, 5 sections, not managed | The configuration application itself |
| `7za.exe` | 265,216 bytes, 7-Zip 16.04 console | Backup/restore of preset archives |
| `7z.dll` | 1,073,664 bytes, 7-Zip 16.04 | Compression plugin |

Additional resources in `Peace.exe`: a compiled help file (119 entries, 79 HTML
topics), theme packages (`theme.ini`, `graphtheme.ini`, PNG control art), icon
sets, translation tables in more than ten languages, 89 `.peace` presets, and a
7-Zip archive of AutoEQ/OPRA correction data (19.8 MB uncompressed).

## 4. Observed functional surface

From the shipped help topics:

- **Equalizer**: up to 31 bands per speaker bank; per band frequency, gain and
  Q; filter shapes peak, low/high pass, low/high shelf (6, 12 dB), band pass,
  notch, all pass, plus Linkwitz-Riley and Butterworth options documented as
  combined responses. Preamp per bank and globally.
- **Speakers**: nine default banks (all, L, R, C, SUB, RL, RR, SL, SR) plus
  user defined combinations; per device equalization; stereo and multichannel.
- **Graph**: 10 Hz–21 kHz response sum for the selected bank, REW SPL overlay,
  frequency response image overlay, preamp inclusion toggle, draggable gain and
  frequency handles.
- **Effects**: crossfeed (Chu Moy, Jan Meier and generic variants), duophonic /
  faux stereo, echo, delay, channel swap and reverse, upmix, 5.1/7.1 downmix,
  mid/side processing, widening by delay.
- **Graphic EQ**: convolution mode writing `GraphicEQ:` instead of biquads.
- **Commands**: raw Equalizer APO command editor, plus a bridge to Equalizer
  APO's own configuration editor for VST plugins.
- **Presets**: save, rename, delete, import/export, a bundled library, and
  additional packs installed by the setup tool.
- **Import**: Equalizer APO configuration files (preamp, biquads, Graphic EQ,
  channel selection, VST plugins), REW filters, AutoEQ data.
- **Export**: Audacity equalizer XML and TXT.
- **Automation**: hotkeys, tray menu switching, per-process and per-device
  configuration attachment, "always active" sets.
- **Settings**: behaviour, colours, sizes, hotkeys, tray, debugging and testing,
  backup/restore, language, appearance.
- **Devices**: selects the Equalizer APO installation and the specific
  configuration file for a device.

## 5. Configuration data model

`.peace` files are INI documents. Sections observed across the 89 shipped
presets:

| Section | Keys |
| --- | --- |
| `[General]` | `Description`, `PreAmp`, `PreAmp1..6`, `Routing`, `Bass Gain/Frequency/Target`, `Treble Gain/Frequency`, `Delay`, `Echo Count/Delay`, `Crossfeed`, `Fake Stereo`, `Fake Stereo Delay/Frequency`, `Swap Speakers`, `Reverse`, `Upmix`, `Downmix 5.1`, `Downmix 7.1`, `GraphicEQ`, `Channels Delay`, `Spatial Shift`, `Stereo Shift` |
| `[Speakers]` | `SpeakerId{n}`, `SpeakerTargets{n}`, `SpeakerName{n}` |
| `[Frequencies]`, `[Frequencies{n}]` | `Frequency1..31` |
| `[Qualities]`, `[Qualities{n}]` | `Quality1..31` |
| `[Gains]`, `[Gains{n}]` | `Gain1..31` |
| `[Filters]`, `[Filters{n}]` | `Filter1..31` (integer shape code) |
| `[Disabled]` | `Disabled1..31` |
| `[Commands]` | `0..n` raw command lines, tab separated |
| `[Configuration]` | `HotKey` |
| `[Surround]` | `Surround On`, levels, delays, widths, `Bass Frequency`, crossovers |
| `[Sliders]` | `Slider1-0..Slider5-2` labels |

Speaker bank `0` uses unsuffixed sections; banks 1..n use the bank index as a
suffix. This was confirmed by parsing every bundled preset.

Observed filter codes and their meaning (from preset descriptions and generated
command strings):

| Code | Shape | Evidence |
| --- | --- | --- |
| 1 | low pass | "Filter low pass 1 kHz" preset |
| 2 | high pass | "Filter high pass 1 kHz" preset |
| 3 | band pass | "Filter band 1 kHz" preset |
| 4 | low shelf | "Filter low shelf 1 kHz" |
| 5 | high shelf | "Filter high shelf 1 kHz" |
| 6 | notch | "Filter notch 1 kHz" |
| 8 / 9 | low / high shelf 12 dB | "Tilt filter" presets |
| 10 / 11 | low / high pass with Q | 15 band HPF/LPF preset |
| 14 / 15 | low / high shelf 6 dB | "Bass Boost" (`Filter5=14` + gain) |
| 12 / 13 | low pass variants | "Ultrasound cut-off" preset; folded onto low pass |

Code 0 (absent key) means peak, which is the default in every untagged band.

Command templates observed verbatim in presets: `Preamp: -3 dB`,
`Copy: L2=0.5*L+0.5*R`, `Channel: L2 R2`, `Filter: ON LP Fc 700 Hz`,
`Delay: 0.26 ms`, `Filter 1: ON HS Fc 650 Hz Gain -30 dB`,
`Copy: L=0.314*L+0.222*C+0.031*SUB+0.268*RL+0.164*RR`, `Stage: post-mix`,
`GraphicEQ:`.

## 6. What PeaceControl reproduces

| Original capability | PeaceControl |
| --- | --- |
| 31 band parametric EQ, all shapes | Yes, 14 shapes mapped to Equalizer APO tokens |
| N speaker banks with channel targets | Yes, unlimited custom banks |
| Global and per bank preamp | Yes |
| Response graph with drag editing | Yes, plus preamp overlay |
| Effects list above | Yes, written as Equalizer APO commands (approximations documented) |
| Graphic EQ mode | Yes |
| Raw commands | Yes, with rendered preview |
| Presets, import, export | Yes; `.peace` interop, JSON library, Audacity export |
| Device/config target selection | Yes, detection plus explicit target |
| Backups, settings, logging | Yes, plus crash recovery drafts |
| Tray icon | Yes: native menu with show/hide, enable, configurations, settings, exit |
| Global hotkeys | Yes: toggle and previous/next configuration, user configurable |
| Process automation | Yes: foreground process rules edited in the Applications window |
| Device attachment automation | No: the configuration target is chosen manually |
| Peak value meter | No: no audio is processed here, so there is nothing to meter |
| VST config editor bridge, update check, translations | Not implemented |

The remaining omissions are deliberate. Device attachment would require
inspecting every audio endpoint; the peak meter needs the original vendor's
metering component or a sample-accurate tap; the VST bridge opens another
application's editor; update checks and translations require a network service
and a translation effort. See `docs/COMPATIBILITY.md`.

## 7. Branding and assets

No icon, image, theme package, help file, translation table, preset file or
string from the original is included in this repository. The interface,
logo, palette, documentation and the nine built-in presets were written for
PeaceControl. The `.peace` format is implemented as a file-format
interoperability layer, and the bundled preset data was used only to derive the
field meanings documented above.
