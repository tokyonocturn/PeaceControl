//! The PeaceControl desktop application.

use crate::engine::Engine;
use crate::theme::{self, Palette};
use crate::{brand, dialogs, graph, platform, widgets};
use eframe::egui::{self, Align, Color32, Layout, RichText, Rounding, Sense, Stroke, Vec2};
use peace_control_core::model::{
    log_bands, FilterType, Preset, MAX_BANDS, MAX_FREQ_HZ, MIN_FREQ_HZ,
};
use peace_control_core::paths::AppPaths;
use peace_control_core::preset_store::PresetStore;
use peace_control_core::settings::{Settings, Theme};
use peace_control_core::{session, VERSION};
use std::path::PathBuf;
use std::time::Instant;

/// Severity of the status line message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Warning,
    Error,
}

impl StatusKind {
    pub fn color(self, palette: &Palette) -> Color32 {
        match self {
            StatusKind::Info => palette.muted,
            StatusKind::Success => palette.ok,
            StatusKind::Warning => palette.warn,
            StatusKind::Error => palette.danger,
        }
    }
}

/// A transient message shown at the bottom of the window.
#[derive(Debug, Clone)]
pub struct Status {
    pub text: String,
    pub kind: StatusKind,
    pub at: Instant,
}

impl Status {
    fn new(text: impl Into<String>, kind: StatusKind) -> Self {
        Status {
            text: text.into(),
            kind,
            at: Instant::now(),
        }
    }

    fn opacity(&self) -> f32 {
        let age = self.at.elapsed().as_secs_f32();
        if age < 6.0 {
            1.0
        } else {
            (1.0 - (age - 6.0) / 3.0).clamp(0.0, 1.0)
        }
    }
}

/// Which secondary windows are open.
#[derive(Debug, Default, Clone)]
pub struct Windows {
    pub effects: bool,
    pub settings: bool,
    pub commands: bool,
    pub import: bool,
    pub about: bool,
    pub backups: bool,
    pub recovery: bool,
    pub help: bool,
    pub confirm_close: bool,
    pub apps: bool,
}

/// Nested action, applied after the interface closure finished borrowing.
enum DeferredAction {
    Load(String),
    Delete(String),
    Duplicate(String),
}

pub struct PeaceApp {
    pub(crate) paths: AppPaths,
    pub(crate) settings: Settings,
    pub(crate) store: PresetStore,
    pub(crate) engine: Engine,
    pub(crate) preset: Preset,
    pub(crate) speaker: usize,
    pub(crate) selected_band: Option<usize>,
    pub(crate) dirty: bool,
    pub(crate) source_path: Option<PathBuf>,
    pub(crate) history: Vec<Preset>,
    pub(crate) redo_stack: Vec<Preset>,
    pub(crate) status: Status,
    pub(crate) windows: Windows,
    pub(crate) palette: Palette,
    pub(crate) search: String,
    pub(crate) preset_files: Vec<peace_control_core::preset_store::StoredPreset>,
    pub(crate) import_text: String,
    pub(crate) import_notes: Vec<String>,
    pub(crate) import_summary: Option<String>,
    pub(crate) import_bands: Option<Vec<peace_control_core::Band>>,
    pub(crate) draft: Option<session::Draft>,
    pub(crate) applied_at: Option<Instant>,
    pub(crate) last_size: Vec2,
    pub(crate) show_preamp: bool,
    pub(crate) export_text: String,
    pub(crate) export_title: String,
    pub(crate) commands_buffer: String,
    pub(crate) last_edit: Instant,
    pub(crate) allow_close: bool,
    pub(crate) shell: Option<crate::tray::Shell>,
    pub(crate) shell_problems: Vec<String>,
    pub(crate) window_visible: bool,
    pub(crate) start_hidden: bool,
    pub(crate) automation: peace_control_core::automation::Tracker,
    pub(crate) last_automation_poll: Instant,
    pub(crate) tooltip_cache: String,
}

impl PeaceApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        paths: AppPaths,
        settings: Settings,
        start_hidden: bool,
    ) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let mut settings = settings;
        settings = settings.sanitized();
        settings.start_with_windows = crate::platform::start_with_windows_enabled();

        let palette = Self::palette_for(&settings, &cc.egui_ctx);
        theme::apply(&cc.egui_ctx, &palette);

        let store = PresetStore::new(paths.presets_dir());
        if let Err(error) = store.seed_library() {
            log::warn!("could not seed the preset library: {error}");
        }

        let mut engine = Engine::new(paths.clone());
        engine.refresh(&settings);

        let mut preset = settings
            .last_preset
            .as_ref()
            .and_then(|name| store.load(name).ok())
            .unwrap_or_else(|| Preset::with_bands(settings.default_band_count));
        if preset.name.trim().is_empty() {
            preset.name = "Untitled".to_string();
        }

        let draft = session::load(&paths.draft_file());
        let has_draft = draft
            .as_ref()
            .map(|draft| draft.preset != preset && draft.age_seconds() < 60 * 60 * 24 * 14)
            .unwrap_or(false);

        let mut app = PeaceApp {
            paths,
            settings,
            store,
            engine,
            preset,
            speaker: 0,
            selected_band: Some(0),
            dirty: false,
            source_path: None,
            history: Vec::new(),
            redo_stack: Vec::new(),
            status: Status::new("Ready", StatusKind::Info),
            windows: Windows::default(),
            palette,
            search: String::new(),
            preset_files: Vec::new(),
            import_text: String::new(),
            import_notes: Vec::new(),
            import_summary: None,
            import_bands: None,
            draft,
            applied_at: None,
            last_size: Vec2::new(1180.0, 760.0),
            show_preamp: false,
            export_text: String::new(),
            export_title: "Export".to_string(),
            commands_buffer: String::new(),
            last_edit: Instant::now(),
            allow_close: false,
            shell: None,
            shell_problems: Vec::new(),
            window_visible: true,
            start_hidden,
            automation: peace_control_core::automation::Tracker::default(),
            last_automation_poll: Instant::now(),
            tooltip_cache: String::new(),
        };
        app.init_shell(&cc.egui_ctx);
        app.windows.recovery = has_draft;
        app.refresh_files();
        app.show_preamp = app.settings.preamp_in_graph;

        if app.engine.connected() {
            app.status("Connected to Equalizer APO", StatusKind::Success);
        } else {
            app.status(
                "Equalizer APO was not detected. Choose its folder in Settings.",
                StatusKind::Warning,
            );
        }
        app
    }

    fn palette_for(settings: &Settings, _ctx: &egui::Context) -> Palette {
        let dark = match settings.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => crate::platform::system_prefers_dark(),
        };
        let mut palette = Palette::for_dark(dark);
        palette.accent =
            Color32::from_rgb(settings.accent[0], settings.accent[1], settings.accent[2]);
        palette.accent_text = if dark {
            Color32::from_rgb(6, 24, 24)
        } else {
            Color32::WHITE
        };
        palette
    }

    pub(crate) fn status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = Status::new(text, kind);
    }

    pub(crate) fn refresh_files(&mut self) {
        self.preset_files = self.store.list();
    }

    pub(crate) fn push_history(&mut self) {
        self.history.push(self.preset.clone());
        if self.history.len() > 100 {
            self.history.remove(0);
        }
        self.redo_stack.clear();
    }

    pub(crate) fn note_edit(&mut self, started: bool) {
        if started {
            self.push_history();
        }
        self.dirty = true;
        self.last_edit = Instant::now();
        if self.settings.apply_automatically && self.applied_at.is_some() {
            // Debounced live apply: only when the user is not mid drag is it
            // safe to write; the interface calls this on release.
        }
    }

    pub(crate) fn undo(&mut self) {
        if let Some(previous) = self.history.pop() {
            self.redo_stack.push(self.preset.clone());
            self.preset = previous;
            self.dirty = true;
            self.clamp_selection();
            self.status("Undone", StatusKind::Info);
        }
    }

    pub(crate) fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.history.push(self.preset.clone());
            self.preset = next;
            self.dirty = true;
            self.clamp_selection();
            self.status("Redone", StatusKind::Info);
        }
    }

    pub(crate) fn clamp_selection(&mut self) {
        self.speaker = self
            .speaker
            .min(self.preset.speakers.len().saturating_sub(1));
        let band_count = self.preset.speakers[self.speaker].bands.len();
        self.selected_band = self
            .selected_band
            .filter(|index| *index < band_count)
            .or(if band_count > 0 { Some(0) } else { None });
    }

    pub(crate) fn load_preset_named(&mut self, name: &str) {
        match self.store.load(name) {
            Ok(preset) => {
                self.preset = preset;
                self.preset.name = name.to_string();
                self.speaker = 0;
                self.selected_band = Some(0);
                self.history.clear();
                self.redo_stack.clear();
                self.dirty = false;
                self.source_path = None;
                self.settings.remember_preset(name);
                let _ = self.settings.save(&self.paths.settings_file());
                self.status(format!("Loaded “{name}”"), StatusKind::Success);
            }
            Err(error) => self.status(format!("Could not load preset: {error}"), StatusKind::Error),
        }
        self.clamp_selection();
    }

    pub(crate) fn new_preset(&mut self, bands: usize) {
        self.push_history();
        self.preset = Preset::with_bands(bands);
        self.preset.name = next_untitled(&self.preset_files);
        self.speaker = 0;
        self.selected_band = Some(0);
        self.dirty = true;
        self.status("New configuration created", StatusKind::Info);
    }

    pub(crate) fn duplicate_preset(&mut self, name: &str) {
        if let Ok(mut preset) = self.store.load(name) {
            preset.name = next_copy(name);
            if self.store.save(&preset).is_ok() {
                self.refresh_files();
                self.load_preset_named(&preset.name.clone());
                self.status("Configuration duplicated", StatusKind::Success);
            }
        }
    }

    pub(crate) fn delete_preset(&mut self, name: &str) {
        if self.store.delete(name).is_ok() {
            self.refresh_files();
            self.status(format!("Deleted “{name}”"), StatusKind::Info);
        }
    }

    pub(crate) fn save_preset(&mut self) {
        if self.preset.name.trim().is_empty() {
            self.preset.name = "Untitled".to_string();
        }
        match self.store.save(&self.preset) {
            Ok(path) => {
                self.dirty = false;
                let _ = session::clear(&self.paths.draft_file());
                self.settings.remember_preset(&self.preset.name);
                let _ = self.settings.save(&self.paths.settings_file());
                self.refresh_files();
                self.status(
                    format!(
                        "Saved to {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    StatusKind::Success,
                );
            }
            Err(error) => self.status(format!("Save failed: {error}"), StatusKind::Error),
        }
    }

    pub(crate) fn save_preset_as(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.status("Choose a name first", StatusKind::Warning);
            return;
        }
        let mut copy = self.preset.clone();
        copy.name = name.to_string();
        match self.store.save(&copy) {
            Ok(_) => {
                self.preset.name = name.to_string();
                self.dirty = false;
                self.refresh_files();
                self.status(format!("Saved as “{name}”"), StatusKind::Success);
            }
            Err(error) => self.status(format!("Save failed: {error}"), StatusKind::Error),
        }
    }

    pub(crate) fn apply_now(&mut self) {
        if self.preset.effects.graphic_eq {
            log::info!("applying Graphic EQ mode");
        }
        match self.engine.apply_state(&self.preset, self.settings.enabled) {
            Ok(path) => {
                self.applied_at = Some(Instant::now());
                self.status(
                    format!(
                        "Applied to {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    StatusKind::Success,
                );
            }
            Err(error) => self.status(format!("Could not apply: {error}"), StatusKind::Error),
        }
    }

    pub(crate) fn autosave_draft(&mut self) {
        if !self.settings.draft_autosave || !self.dirty {
            return;
        }
        if self.last_edit.elapsed().as_secs() < 2 {
            return;
        }
        let source = self
            .source_path
            .as_ref()
            .map(|path| path.display().to_string());
        let _ = session::save(&self.paths.draft_file(), &self.preset, source.as_deref());
    }

    pub(crate) fn add_speaker(&mut self) {
        self.push_history();
        let targets = if self.preset.speakers.len() % 2 == 0 {
            "L"
        } else {
            "R"
        };
        let index = self.preset.add_speaker(targets);
        self.speaker = index;
        self.selected_band = None;
        self.dirty = true;
    }

    pub(crate) fn remove_speaker(&mut self) {
        if self.preset.speakers.len() <= 1 {
            self.status("Keep at least one speaker bank", StatusKind::Warning);
            return;
        }
        self.push_history();
        self.preset.speakers.remove(self.speaker);
        self.speaker = self.speaker.saturating_sub(1);
        self.selected_band = Some(0);
        self.dirty = true;
    }

    pub(crate) fn export_config_to(&mut self, path: &std::path::Path) {
        let text = self.engine.preview(&self.preset);
        match peace_control_core::fsutil::write_atomic(path, &text) {
            Ok(()) => self.status("Equalizer APO configuration exported", StatusKind::Success),
            Err(error) => self.status(format!("Export failed: {error}"), StatusKind::Error),
        }
    }

    pub(crate) fn export_peace_to(&mut self, path: &std::path::Path) {
        let mut export = self.preset.clone();
        if let Some(stem) = path.file_stem() {
            export.name = stem.to_string_lossy().to_string();
        }
        match peace_control_core::peace_ini::save_preset_file(path, &export) {
            Ok(()) => self.status("Interoperable .peace file exported", StatusKind::Success),
            Err(error) => self.status(format!("Export failed: {error}"), StatusKind::Error),
        }
    }

    pub(crate) fn import_from_path(&mut self, path: &std::path::Path) {
        let extension = path
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if extension == "peace" {
            match peace_control_core::peace_ini::load_preset_file(path) {
                Ok(preset) => {
                    self.push_history();
                    self.preset = preset;
                    self.speaker = 0;
                    self.clamp_selection();
                    self.dirty = true;
                    self.status("Imported .peace configuration", StatusKind::Success);
                }
                Err(error) => self.status(format!("Import failed: {error}"), StatusKind::Error),
            }
            return;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.import_text = text;
                self.analyse_import();
                self.windows.import = true;
            }
            Err(error) => self.status(
                format!("Could not read the file: {error}"),
                StatusKind::Error,
            ),
        }
    }

    pub(crate) fn analyse_import(&mut self) {
        self.import_notes.clear();
        self.import_bands = None;
        self.import_summary = None;
        let text = self.import_text.clone();
        if text.trim().is_empty() {
            return;
        }
        if let Some(report) = peace_control_core::parse::parse_config(&text) {
            self.import_summary = Some(format!(
                "{} speaker bank(s) · {}",
                report.preset.speakers.len(),
                report.summary()
            ));
            self.import_notes = report.notes;
            self.import_bands = Some(
                report
                    .preset
                    .speakers
                    .first()
                    .map(|speaker| speaker.bands.clone())
                    .unwrap_or_default(),
            );
            return;
        }
        match peace_control_core::autoeq::import_text(&text, MAX_BANDS) {
            Ok(result) => {
                self.import_summary = Some(format!(
                    "{} · {} points",
                    result.format.label(),
                    result.points
                ));
                self.import_notes = result.notes;
                self.import_bands = Some(result.bands);
            }
            Err(error) => {
                self.import_notes = vec![error.to_string()];
            }
        }
    }

    pub(crate) fn apply_import(&mut self, replace: bool) {
        let text = self.import_text.clone();
        if text.trim().is_empty() {
            self.status("Nothing to import", StatusKind::Warning);
            return;
        }
        if let Some(report) = peace_control_core::parse::parse_config(&text) {
            if replace {
                self.push_history();
                self.preset = report.preset;
                if let Some(stem) = self.source_path.as_ref().and_then(|path| path.file_stem()) {
                    self.preset.name = stem.to_string_lossy().to_string();
                }
            } else {
                self.push_history();
                let bands = report
                    .preset
                    .speakers
                    .first()
                    .map(|speaker| speaker.bands.clone())
                    .unwrap_or_default();
                self.preset.speakers[self.speaker].bands = bands;
            }
            self.speaker = 0;
            self.clamp_selection();
            self.dirty = true;
            self.windows.import = false;
            self.status("Import applied", StatusKind::Success);
            return;
        }
        match peace_control_core::autoeq::import_text(&text, MAX_BANDS) {
            Ok(result) => {
                self.push_history();
                self.preset.speakers[self.speaker].bands = result.bands;
                self.clamp_selection();
                self.dirty = true;
                self.windows.import = false;
                self.status("Measurement curve imported", StatusKind::Success);
            }
            Err(error) => self.status(format!("Import failed: {error}"), StatusKind::Error),
        }
    }

    pub(crate) fn reset_settings(&mut self) {
        self.settings = Settings {
            equalizer_apo_dir: self.settings.equalizer_apo_dir.clone(),
            target_config: self.settings.target_config.clone(),
            ..Settings::default()
        };
        self.palette = Self::palette_for(&self.settings, &egui::Context::default());
        self.status("Settings reset to defaults", StatusKind::Info);
    }

    pub(crate) fn pick_preset_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Configurations", &["peace", "txt", "csv", "json"])
            .set_title("Open configuration")
            .pick_file()
        {
            self.import_from_path(&path);
        }
    }

    pub(crate) fn export_peace_dialog(&mut self) {
        let default_name = format!("{}.peace", self.preset.name);
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Peace configuration", &["peace"])
            .set_file_name(default_name)
            .save_file()
        {
            self.export_peace_to(&path);
        }
    }

    pub(crate) fn export_config_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Equalizer APO configuration", &["txt"])
            .set_file_name("config.txt")
            .save_file()
        {
            self.export_config_to(&path);
        }
    }

    pub(crate) fn export_audacity_dialog(&mut self, xml: bool) {
        let (name, extension, filter) = if xml {
            ("equalization.xml", "xml", "Audacity XML")
        } else {
            ("equalization.txt", "txt", "Audacity text")
        };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(filter, &[extension])
            .set_file_name(name)
            .save_file()
        {
            let text = if xml {
                peace_control_core::audacity::to_audacity_xml(&self.preset, self.speaker)
            } else {
                peace_control_core::audacity::to_audacity_txt(&self.preset, self.speaker)
            };
            match peace_control_core::fsutil::write_atomic(&path, &text) {
                Ok(()) => self.status("Audacity curve exported", StatusKind::Success),
                Err(error) => self.status(format!("Export failed: {error}"), StatusKind::Error),
            }
        }
    }

    pub(crate) fn backup_now(&mut self) {
        match self.engine.snapshot(self.settings.max_backups) {
            Ok(Some(snapshot)) => self.status(
                format!("Snapshot saved ({} files)", snapshot.files),
                StatusKind::Success,
            ),
            Ok(None) => self.status(
                "No Equalizer APO configuration folder to back up",
                StatusKind::Warning,
            ),
            Err(error) => self.status(format!("Backup failed: {error}"), StatusKind::Error),
        }
    }

    pub(crate) fn add_band(&mut self) {
        if self.preset.speakers[self.speaker].bands.len() >= MAX_BANDS {
            self.status(
                format!("A bank can hold at most {MAX_BANDS} bands"),
                StatusKind::Warning,
            );
            return;
        }
        self.push_history();
        let last = self.preset.speakers[self.speaker]
            .bands
            .last()
            .map(|band| band.freq_hz)
            .unwrap_or(1000.0);
        let freq = (last * 1.5).min(MAX_FREQ_HZ);
        let bands = &mut self.preset.speakers[self.speaker].bands;
        bands.push(peace_control_core::Band::new(freq, 0.0, 1.41));
        bands.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
        self.dirty = true;
    }

    pub(crate) fn remove_band(&mut self) {
        let count = self.preset.speakers[self.speaker].bands.len();
        if count <= 1 {
            self.status("Keep at least one band", StatusKind::Warning);
            return;
        }
        if let Some(index) = self.selected_band {
            self.push_history();
            self.preset.speakers[self.speaker].bands.remove(index);
            let new_count = self.preset.speakers[self.speaker].bands.len();
            self.selected_band = Some(index.min(new_count.saturating_sub(1)));
            self.dirty = true;
        }
    }

    pub(crate) fn set_band_grid(&mut self, kind: Grid) {
        self.push_history();
        let speaker = &mut self.preset.speakers[self.speaker];
        speaker.bands = match kind {
            Grid::Log(count) => log_bands(count),
            Grid::ThirdOctave => peace_control_core::model::third_octave_frequencies()
                .into_iter()
                .map(|freq| peace_control_core::Band::new(freq, 0.0, 4.36))
                .collect(),
        };
        self.selected_band = Some(0);
        self.dirty = true;
    }

    /// Opens the raw commands window with a fresh editing buffer.
    pub(crate) fn open_commands(&mut self) {
        self.commands_buffer = self.preset.commands.join("\n");
        self.windows.commands = true;
    }

    pub(crate) fn apply_theme(&mut self, ctx: &egui::Context) {
        self.palette = Self::palette_for(&self.settings, ctx);
        theme::apply(ctx, &self.palette);
    }
}

/// Band grid presets available in the interface.
#[derive(Debug, Clone, Copy)]
pub enum Grid {
    Log(usize),
    ThirdOctave,
}

fn next_untitled(existing: &[peace_control_core::preset_store::StoredPreset]) -> String {
    let mut index = 1;
    loop {
        let candidate = format!("Untitled {index}");
        if !existing.iter().any(|entry| entry.name == candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn next_copy(name: &str) -> String {
    format!("{name} copy")
}

impl eframe::App for PeaceApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Closing can keep running in the tray instead of quitting.
        if ctx.input(|input| input.viewport().close_requested()) && !self.allow_close {
            if self.settings.close_to_tray {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.set_window_visible(ctx, false);
                self.status("Still running in the tray", StatusKind::Info);
            } else if self.dirty && self.settings.confirm_on_exit {
                // Ask before losing unsaved work.
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.windows.confirm_close = true;
            }
        }

        self.poll_shell(ctx);
        self.poll_automation(ctx);

        // Hiding instead of minimising keeps the taskbar clean.
        if self.settings.minimize_to_tray
            && self.window_visible
            && ctx.input(|input| input.viewport().minimized == Some(true))
        {
            self.set_window_visible(ctx, false);
        }

        let shortcuts = ctx.input(|input| ShortcutState {
            save: input.modifiers.command && input.key_pressed(egui::Key::S),
            apply: input.modifiers.command && input.key_pressed(egui::Key::E),
            undo: input.modifiers.command
                && !input.modifiers.shift
                && input.key_pressed(egui::Key::Z),
            redo: input.modifiers.command
                && ((input.modifiers.shift && input.key_pressed(egui::Key::Z))
                    || input.key_pressed(egui::Key::Y)),
            open: input.modifiers.command && input.key_pressed(egui::Key::O),
            help: input.key_pressed(egui::Key::F1),
        });

        if shortcuts.save {
            self.save_preset();
        }
        if shortcuts.apply {
            self.apply_now();
        }
        if shortcuts.undo {
            self.undo();
        }
        if shortcuts.redo {
            self.redo();
        }
        if shortcuts.open {
            self.pick_preset_file();
        }
        if shortcuts.help {
            self.windows.help = true;
        }

        self.last_size = ctx.screen_rect().size();
        self.autosave_draft();

        self.top_bar(ctx);
        dialogs::show_all(self, ctx);

        egui::TopBottomPanel::bottom("status-bar")
            .frame(
                egui::Frame::none()
                    .fill(self.palette.panel)
                    .inner_margin(egui::Margin::symmetric(14.0, 6.0)),
            )
            .show(ctx, |ui| self.status_bar(ui));

        egui::SidePanel::left("presets-panel")
            .resizable(true)
            .default_width(262.0)
            .width_range(220.0..=340.0)
            .frame(
                egui::Frame::none()
                    .fill(self.palette.panel)
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0)),
            )
            .show(ctx, |ui| self.left_panel(ui));

        egui::SidePanel::right("inspector-panel")
            .resizable(true)
            .default_width(288.0)
            .width_range(250.0..=380.0)
            .frame(
                egui::Frame::none()
                    .fill(self.palette.panel)
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0)),
            )
            .show(ctx, |ui| self.right_panel(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(self.palette.bg)
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0)),
            )
            .show(ctx, |ui| self.center_panel(ui));

        self.toast(ctx);

        if self.settings.apply_automatically
            && self.settings.enabled
            && self.dirty
            && self.last_edit.elapsed().as_millis() > 500
            && self.applied_at.is_some()
            && !self.windows.effects
        {
            // Only rewrite while the pointer is up, so dragging never hammers
            // the audio engine with intermediate files.
            if !ctx.input(|input| input.pointer.primary_down()) {
                if let Ok(path) = self.engine.apply_state(&self.preset, self.settings.enabled) {
                    log::debug!("live apply to {}", path.display());
                    self.applied_at = Some(Instant::now());
                    self.last_edit = Instant::now() + std::time::Duration::from_secs(60);
                }
            }
        }

        let _ = frame;
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.release_shell();
        self.settings.window_width = self.last_size.x;
        self.settings.window_height = self.last_size.y;
        let _ = self.settings.save(&self.paths.settings_file());
        if self.settings.backup_on_exit {
            let _ = self.engine.snapshot(self.settings.max_backups);
        }
        if self.dirty {
            let source = self
                .source_path
                .as_ref()
                .map(|path| path.display().to_string());
            let _ = session::save(&self.paths.draft_file(), &self.preset, source.as_deref());
        } else {
            let _ = session::clear(&self.paths.draft_file());
        }
        log::info!("PeaceControl exiting");
    }
}

struct ShortcutState {
    save: bool,
    apply: bool,
    undo: bool,
    redo: bool,
    open: bool,
    help: bool,
}

impl PeaceApp {
    fn top_bar(&mut self, ctx: &egui::Context) {
        let palette = self.palette;
        egui::TopBottomPanel::top("top-bar")
            .exact_height(62.0)
            .frame(
                egui::Frame::none()
                    .fill(palette.panel)
                    .inner_margin(egui::Margin::symmetric(14.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    brand::wordmark(ui, &palette, false);
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(12.0);

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let name_response = ui.add(
                                egui::TextEdit::singleline(&mut self.preset.name)
                                    .desired_width(230.0)
                                    .font(egui::TextStyle::Body)
                                    .frame(false),
                            );
                            if name_response.changed() {
                                self.note_edit(false);
                            }
                            if self.dirty {
                                ui.label(RichText::new("●").size(9.0).color(palette.warn))
                                    .on_hover_text("Unsaved changes");
                            }
                        });
                        ui.label(
                            RichText::new(if self.preset.description.is_empty() {
                                "No description".to_string()
                            } else {
                                self.preset.description.clone()
                            })
                            .size(10.5)
                            .color(palette.muted),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let ready = self.engine.ready();
                        if widgets::primary_button(ui, "Apply", ready, &palette).clicked() {
                            self.apply_now();
                        }
                        let eq_on = self.settings.enabled;
                        if widgets::ghost_button(
                            ui,
                            if eq_on { "EQ on" } else { "EQ bypassed" },
                            &palette,
                        )
                        .on_hover_text("Enable or bypass the equalizer everywhere")
                        .clicked()
                        {
                            self.set_enabled(!eq_on);
                        }
                        if widgets::ghost_button(ui, "Tray", &palette)
                            .on_hover_text("Hide the window and keep running in the tray")
                            .clicked()
                        {
                            self.set_window_visible(ctx, false);
                        }
                        if widgets::ghost_button(ui, "Save", &palette).clicked() {
                            self.save_preset();
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("⚙").size(15.0).color(palette.text),
                                )
                                .fill(palette.card)
                                .rounding(Rounding::same(8.0))
                                .min_size(Vec2::new(32.0, 30.0)),
                            )
                            .on_hover_text("Settings")
                            .clicked()
                        {
                            self.windows.settings = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("?").size(13.0).color(palette.text),
                                )
                                .fill(palette.card)
                                .rounding(Rounding::same(8.0))
                                .min_size(Vec2::new(32.0, 30.0)),
                            )
                            .on_hover_text("Help and diagnostics")
                            .clicked()
                        {
                            self.windows.help = true;
                        }
                        ui.add_space(6.0);

                        let (color, text) = if self.engine.connected() {
                            (palette.ok, "Equalizer APO")
                        } else {
                            (palette.warn, "Not detected")
                        };
                        ui.vertical(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                widgets::badge(ui, text, color, &palette);
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(self.engine.status_text())
                                        .size(10.0)
                                        .monospace()
                                        .color(palette.muted),
                                );
                            });
                        });
                    });
                });
            });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        widgets::section_title(ui, "Configurations", &palette);
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search")
                    .desired_width(ui.available_width() - 70.0),
            );
            if widgets::ghost_button(ui, "+ New", &palette).clicked() {
                let count = self.settings.default_band_count;
                self.new_preset(count);
            }
        });
        ui.add_space(6.0);

        let mut action: Option<DeferredAction> = None;
        let search = self.search.to_ascii_lowercase();
        let filter_files = |files: &Vec<peace_control_core::preset_store::StoredPreset>,
                            search: &str| {
            files
                .iter()
                .filter(|entry| {
                    search.is_empty()
                        || entry.name.to_ascii_lowercase().contains(search)
                        || entry.description.to_ascii_lowercase().contains(search)
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        let files = filter_files(&self.preset_files, &search);

        egui::ScrollArea::vertical()
            .id_salt("preset-list")
            .max_height(ui.available_height() * 0.56)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for entry in &files {
                    let is_current = entry.name == self.preset.name;
                    let response = ui
                        .push_id(("preset", &entry.name), |ui| {
                            let frame = egui::Frame::none()
                                .fill(if is_current {
                                    palette.accent_alpha(26)
                                } else {
                                    palette.card
                                })
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    if is_current {
                                        palette.accent_alpha(120)
                                    } else {
                                        palette.border
                                    },
                                ))
                                .rounding(theme::SMALL_RADIUS)
                                .inner_margin(egui::Margin::symmetric(8.0, 6.0));
                            frame
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&entry.name)
                                                .size(12.0)
                                                .color(if is_current {
                                                    palette.accent
                                                } else {
                                                    palette.text
                                                })
                                                .strong(),
                                        );
                                    });
                                    if !entry.description.is_empty() {
                                        ui.label(
                                            RichText::new(&entry.description)
                                                .size(9.5)
                                                .color(palette.muted),
                                        );
                                    }
                                })
                                .response
                        })
                        .inner
                        .interact(Sense::click());

                    response.context_menu(|ui| {
                        if ui.button("Load").clicked() {
                            action = Some(DeferredAction::Load(entry.name.clone()));
                            ui.close_menu();
                        }
                        if ui.button("Duplicate").clicked() {
                            action = Some(DeferredAction::Duplicate(entry.name.clone()));
                            ui.close_menu();
                        }
                        if ui.button("Delete").clicked() {
                            action = Some(DeferredAction::Delete(entry.name.clone()));
                            ui.close_menu();
                        }
                    });

                    if response.clicked() {
                        action = Some(DeferredAction::Load(entry.name.clone()));
                    }
                    ui.add_space(4.0);
                }
                if files.is_empty() {
                    ui.label(
                        RichText::new("No configuration matches the search")
                            .size(11.0)
                            .color(palette.muted),
                    );
                }
            });

        if let Some(action) = action {
            match action {
                DeferredAction::Load(name) => self.load_preset_named(&name),
                DeferredAction::Delete(name) => self.delete_preset(&name),
                DeferredAction::Duplicate(name) => self.duplicate_preset(&name),
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);

        widgets::section_title(ui, "Speaker banks", &palette);
        ui.add_space(2.0);

        let mut select: Option<usize> = None;
        for (index, speaker) in self.preset.speakers.iter().enumerate() {
            let selected = index == self.speaker;
            let flat = speaker.is_flat();
            let label = format!("{}{}", speaker.name, if flat { "" } else { "  *" });
            if ui
                .selectable_label(
                    selected,
                    RichText::new(label).size(12.0).color(if selected {
                        palette.accent
                    } else {
                        palette.text
                    }),
                )
                .on_hover_text(format!("Channels: {}", speaker.targets))
                .clicked()
            {
                select = Some(index);
            }
        }
        if let Some(index) = select {
            self.speaker = index;
            self.selected_band = Some(0);
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if widgets::ghost_button(ui, "+ Add", &palette).clicked() {
                self.add_speaker();
            }
            if widgets::ghost_button(ui, "– Remove", &palette).clicked() {
                self.remove_speaker();
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);
        widgets::section_title(ui, "Library", &palette);
        ui.add_space(4.0);
        if widgets::ghost_button(ui, "Import filters or curve…", &palette)
            .on_hover_text("Equalizer APO config, REW export, AutoEQ or CSV")
            .clicked()
        {
            self.windows.import = true;
        }
        if widgets::ghost_button(ui, "Open configuration file…", &palette).clicked() {
            self.pick_preset_file();
        }
        if widgets::ghost_button(ui, "Export .peace…", &palette).clicked() {
            self.export_peace_dialog();
        }
        if widgets::ghost_button(ui, "Save as copy", &palette)
            .on_hover_text("Keep this configuration and start a renamed copy")
            .clicked()
        {
            let name = format!("{} copy", self.preset.name);
            self.save_preset_as(&name);
        }
    }

    fn center_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let speaker_index = self.speaker;
        let speaker_name = self.preset.speakers[speaker_index].name.clone();
        let targets = self.preset.speakers[speaker_index].targets.clone();
        let band_count = self.preset.speakers[speaker_index].bands.len();
        let preamp = self.preset.speakers[speaker_index].preamp_db;

        // Preamp and quick actions.
        egui::Frame::none()
            .fill(palette.panel)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .rounding(theme::CARD_RADIUS)
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Preamp").size(12.0).color(palette.muted));
                    let mut value = preamp;
                    if ui
                        .add(
                            egui::DragValue::new(&mut value)
                                .speed(0.1)
                                .range(-40.0..=12.0)
                                .suffix(" dB")
                                .fixed_decimals(1),
                        )
                        .changed()
                    {
                        self.push_history();
                        self.preset.speakers[speaker_index].preamp_db = value;
                        self.dirty = true;
                        self.last_edit = Instant::now();
                    }
                    if ui
                        .add(egui::Slider::new(&mut value, -24.0..=12.0).show_value(false))
                        .changed()
                    {
                        self.preset.speakers[speaker_index].preamp_db = value;
                        self.dirty = true;
                        self.last_edit = Instant::now();
                    }
                    if ui
                        .small_button("reset")
                        .on_hover_text("Set the preamp back to 0 dB")
                        .clicked()
                    {
                        self.push_history();
                        self.preset.speakers[speaker_index].preamp_db = 0.0;
                        self.dirty = true;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::ghost_button(ui, "Flatten", &palette)
                            .on_hover_text("Reset every band to 0 dB")
                            .clicked()
                        {
                            self.push_history();
                            self.preset.flatten();
                            self.dirty = true;
                        }
                        if widgets::ghost_button(ui, "Effects", &palette)
                            .on_hover_text("Crossfeed, stereo tools, delays and surround")
                            .clicked()
                        {
                            self.windows.effects = true;
                        }
                        if widgets::ghost_button(ui, "Commands", &palette)
                            .on_hover_text("Raw Equalizer APO command lines")
                            .clicked()
                        {
                            self.open_commands();
                        }
                        if widgets::ghost_button(ui, "Apps", &palette)
                            .on_hover_text("Switch configurations per application")
                            .clicked()
                        {
                            self.windows.apps = true;
                        }
                        let active = self.preset.effects.active_count();
                        if active > 0 {
                            widgets::chip(ui, &format!("{active} effect(s)"), &palette, true);
                        }
                        widgets::chip(ui, &format!("{band_count} bands"), &palette, false);
                    });
                });
            });

        ui.add_space(10.0);

        // Graph.
        let graph_height = (ui.available_height() - 190.0).max(200.0);
        egui::Frame::none()
            .fill(palette.panel)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .rounding(theme::CARD_RADIUS)
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{speaker_name} · {targets}"))
                            .size(12.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .checkbox(&mut self.show_preamp, "include preamp")
                            .changed()
                        {
                            self.settings.preamp_in_graph = self.show_preamp;
                        }
                        widgets::chip(ui, "predicted response", &palette, false);
                    });
                });

                let selected = self.selected_band;
                let mut chosen = selected;
                let show_preamp = self.show_preamp;
                let range = self.settings.graph_range_db;
                let outcome = graph::show(
                    ui,
                    &mut self.preset.speakers[speaker_index].bands,
                    &mut chosen,
                    preamp,
                    show_preamp,
                    range,
                    graph_height,
                    &palette,
                );
                if outcome.started_edit {
                    self.push_history();
                }
                if outcome.changed {
                    self.dirty = true;
                    self.last_edit = Instant::now();
                }
                if chosen != self.selected_band {
                    self.selected_band = chosen;
                }
            });

        ui.add_space(10.0);

        // Band sliders.
        egui::Frame::none()
            .fill(palette.panel)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .rounding(theme::CARD_RADIUS)
            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Bands").size(11.0).color(palette.muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::ghost_button(ui, "+", &palette)
                            .on_hover_text("Add a band")
                            .clicked()
                        {
                            self.add_band();
                        }
                        if widgets::ghost_button(ui, "–", &palette)
                            .on_hover_text("Remove the selected band")
                            .clicked()
                        {
                            self.remove_band();
                        }
                    });
                });
                let mut selected = self.selected_band;
                let outcome = widgets::band_strip(
                    ui,
                    &mut self.preset.speakers[speaker_index].bands,
                    &mut selected,
                    &palette,
                    132.0,
                );
                if outcome.started_edit {
                    self.push_history();
                }
                if let Some(index) = outcome.reset_band {
                    self.push_history();
                    if let Some(band) = self.preset.speakers[speaker_index].bands.get_mut(index) {
                        band.gain_db = 0.0;
                        self.dirty = true;
                    }
                }
                if outcome.changed {
                    self.dirty = true;
                    self.last_edit = Instant::now();
                }
                if selected != self.selected_band {
                    self.selected_band = selected;
                }
            });
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let speaker_index = self.speaker;

        widgets::section_title(ui, "Preset", &palette);
        ui.add_space(4.0);
        egui::Frame::none()
            .fill(palette.panel)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .rounding(theme::CARD_RADIUS)
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.label(RichText::new("Description").size(10.5).color(palette.muted));
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.preset.description)
                            .hint_text("What this configuration is for")
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    self.dirty = true;
                    self.last_edit = Instant::now();
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Global preamp")
                        .size(10.5)
                        .color(palette.muted),
                );
                let mut global = self.preset.preamp_db;
                if ui
                    .add(
                        egui::DragValue::new(&mut global)
                            .speed(0.1)
                            .range(-40.0..=12.0)
                            .suffix(" dB")
                            .fixed_decimals(1),
                    )
                    .changed()
                {
                    self.push_history();
                    self.preset.preamp_db = global;
                    self.dirty = true;
                    self.last_edit = Instant::now();
                }
                ui.label(
                    RichText::new("Applied once, before every speaker bank.")
                        .size(9.5)
                        .color(palette.muted),
                );
            });
        ui.add_space(10.0);

        widgets::section_title(ui, "Selected band", &palette);
        ui.add_space(4.0);

        let band_index = self.selected_band;
        let mut changed = false;
        let mut reset = false;
        let mut solo = false;

        egui::Frame::none()
            .fill(palette.panel)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .rounding(theme::CARD_RADIUS)
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| match band_index {
                Some(index) => {
                    if let Some(band) = self.preset.speakers[speaker_index].bands.get_mut(index) {
                        egui::Grid::new("band-editor")
                            .num_columns(2)
                            .spacing([8.0, 6.0])
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Frequency").size(11.0).color(palette.muted),
                                );
                                let mut freq = band.freq_hz;
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut freq)
                                            .speed(1.0)
                                            .range(MIN_FREQ_HZ..=MAX_FREQ_HZ)
                                            .suffix(" Hz")
                                            .fixed_decimals(0),
                                    )
                                    .changed()
                                {
                                    band.freq_hz = freq;
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label(RichText::new("Gain").size(11.0).color(palette.muted));
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut band.gain_db)
                                            .speed(0.1)
                                            .range(-30.0..=30.0)
                                            .suffix(" dB")
                                            .fixed_decimals(1),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label(RichText::new("Quality").size(11.0).color(palette.muted));
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut band.q)
                                            .speed(0.01)
                                            .range(0.1..=40.0)
                                            .fixed_decimals(2),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label(RichText::new("Filter").size(11.0).color(palette.muted));
                                egui::ComboBox::from_id_salt("filter-type")
                                    .selected_text(band.filter.label())
                                    .width(150.0)
                                    .show_ui(ui, |ui| {
                                        for filter in FilterType::ALL {
                                            if ui
                                                .selectable_value(
                                                    &mut band.filter,
                                                    filter,
                                                    filter.label(),
                                                )
                                                .clicked()
                                            {
                                                changed = true;
                                            }
                                        }
                                    });
                                ui.end_row();

                                ui.label(RichText::new("Enabled").size(11.0).color(palette.muted));
                                if widgets::switch(ui, &mut band.enabled, &palette).clicked() {
                                    changed = true;
                                }
                                ui.end_row();
                            });

                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(band.filter.hint())
                                .size(9.5)
                                .color(palette.muted),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if widgets::ghost_button(ui, "Reset band", &palette).clicked() {
                                reset = true;
                            }
                            if widgets::ghost_button(ui, "Solo", &palette)
                                .on_hover_text(
                                    "Hear only this band; press again to restore the rest",
                                )
                                .clicked()
                            {
                                solo = true;
                            }
                        });
                    } else {
                        ui.label(
                            RichText::new("Select a band on the graph")
                                .size(11.0)
                                .color(palette.muted),
                        );
                    }
                }
                None => {
                    ui.label(
                        RichText::new("Select a band on the graph")
                            .size(11.0)
                            .color(palette.muted),
                    );
                }
            });

        if changed {
            self.push_history();
            self.dirty = true;
            self.last_edit = Instant::now();
        }
        if reset {
            self.push_history();
            if let Some(index) = band_index {
                if let Some(band) = self.preset.speakers[speaker_index].bands.get_mut(index) {
                    band.gain_db = 0.0;
                    band.filter = FilterType::Peak;
                    self.dirty = true;
                }
            }
        }
        if solo {
            self.push_history();
            if let Some(index) = band_index {
                if self.preset.toggle_solo(speaker_index, index) {
                    self.status("Solo: only this band is active", StatusKind::Info);
                } else {
                    self.status("All bands are active again", StatusKind::Info);
                }
                self.dirty = true;
            }
        }

        ui.add_space(10.0);
        widgets::section_title(ui, "Channel", &palette);
        ui.add_space(4.0);
        egui::Frame::none()
            .fill(palette.panel)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .rounding(theme::CARD_RADIUS)
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.label(RichText::new("Name").size(10.5).color(palette.muted));
                if ui
                    .text_edit_singleline(&mut self.preset.speakers[speaker_index].name)
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add_space(4.0);
                ui.label(RichText::new("Channels").size(10.5).color(palette.muted));
                let targets = &mut self.preset.speakers[speaker_index].targets;
                if ui
                    .add(
                        egui::TextEdit::singleline(targets)
                            .hint_text("L R, SUB, L C R, SL SR…")
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    self.dirty = true;
                }
                ui.label(
                    RichText::new("Space separated Equalizer APO channel names, or all.")
                        .size(9.5)
                        .color(palette.muted),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    widgets::chip(
                        ui,
                        &self.preset.speakers[speaker_index].targets,
                        &palette,
                        true,
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .small_button("copy from All")
                            .on_hover_text("Copy the first bank's bands here")
                            .clicked()
                            && speaker_index != 0
                        {
                            self.push_history();
                            self.preset.copy_bands(0, speaker_index);
                            self.dirty = true;
                        }
                    });
                });
            });

        ui.add_space(10.0);
        widgets::section_title(ui, "Band layout", &palette);
        ui.add_space(4.0);
        let mut grid: Option<Grid> = None;
        ui.horizontal_wrapped(|ui| {
            if widgets::ghost_button(ui, "13", &palette)
                .on_hover_text("Octave grid")
                .clicked()
            {
                grid = Some(Grid::Log(13));
            }
            if widgets::ghost_button(ui, "15", &palette).clicked() {
                grid = Some(Grid::Log(15));
            }
            if widgets::ghost_button(ui, "31", &palette)
                .on_hover_text("Third octave")
                .clicked()
            {
                grid = Some(Grid::Log(31));
            }
            if widgets::ghost_button(ui, "1/3 oct", &palette).clicked() {
                grid = Some(Grid::ThirdOctave);
            }
        });
        if let Some(grid) = grid {
            self.set_band_grid(grid);
            self.status("Band layout regenerated", StatusKind::Info);
        }

        ui.add_space(10.0);
        widgets::section_title(ui, "Tools", &palette);
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if widgets::ghost_button(ui, "−3 dB", &palette).clicked() {
                self.push_history();
                self.preset.shift_gains(-3.0);
                self.dirty = true;
            }
            if widgets::ghost_button(ui, "+3 dB", &palette).clicked() {
                self.push_history();
                self.preset.shift_gains(3.0);
                self.dirty = true;
            }
            if widgets::ghost_button(ui, "Compress", &palette)
                .on_hover_text("Scale every gain to 70%")
                .clicked()
            {
                self.push_history();
                self.preset.scale_gains(0.7);
                self.dirty = true;
            }
            if widgets::ghost_button(ui, "Expand", &palette)
                .on_hover_text("Scale every gain to 140%")
                .clicked()
            {
                self.push_history();
                self.preset.scale_gains(1.4);
                self.dirty = true;
            }
        });

        ui.add_space(10.0);
        widgets::section_title(ui, "Export", &palette);
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if widgets::ghost_button(ui, "config.txt", &palette).clicked() {
                self.export_config_dialog();
            }
            if widgets::ghost_button(ui, "Audacity XML", &palette).clicked() {
                self.export_audacity_dialog(true);
            }
            if widgets::ghost_button(ui, "Audacity TXT", &palette).clicked() {
                self.export_audacity_dialog(false);
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        ui.horizontal(|ui| {
            brand::status_dot(ui, self.status.kind.color(&palette), 10.0);
            ui.label(
                RichText::new(&self.status.text).size(11.0).color(
                    self.status
                        .kind
                        .color(&palette)
                        .gamma_multiply(self.status.opacity().max(0.55)),
                ),
            );

            if self.dirty {
                widgets::chip(ui, "unsaved", &palette, true);
            }
            if self.preset.effects.graphic_eq {
                widgets::chip(ui, "Graphic EQ", &palette, true);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("v{VERSION}"))
                        .size(10.0)
                        .color(palette.muted),
                );
                ui.separator();
                if ui
                    .add(
                        egui::Label::new(
                            RichText::new("data folder").size(10.0).color(palette.muted),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_text(self.paths.root().display().to_string())
                    .clicked()
                {
                    platform::reveal(self.paths.root());
                }
                ui.separator();
                if ui
                    .add(
                        egui::Label::new(
                            RichText::new(self.engine.status_text())
                                .size(10.0)
                                .monospace()
                                .color(palette.muted),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_text("Open Equalizer APO configuration folder")
                    .clicked()
                {
                    if let Some(dir) = &self.engine.config_dir {
                        platform::reveal(dir);
                    } else if let Some(dir) = &self.engine.apo_dir {
                        platform::reveal(dir);
                    }
                }
                if let Some(applied) = self.applied_at {
                    let seconds = applied.elapsed().as_secs();
                    ui.label(
                        RichText::new(format!("applied {seconds}s ago"))
                            .size(10.0)
                            .color(palette.muted),
                    );
                }
            });
        });
    }

    fn toast(&mut self, ctx: &egui::Context) {
        let opacity = self.status.opacity();
        if opacity <= 0.02 || self.status.kind == StatusKind::Info {
            return;
        }
        let palette = self.palette;
        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -46.0])
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(palette.card.gamma_multiply(opacity))
                    .stroke(Stroke::new(
                        1.0_f32,
                        self.status.kind.color(&palette).gamma_multiply(opacity),
                    ))
                    .rounding(Rounding::same(9.0))
                    .inner_margin(egui::Margin::symmetric(14.0, 8.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&self.status.text)
                                .size(11.5)
                                .color(palette.text.gamma_multiply(opacity)),
                        );
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untitled_names_skip_existing_entries() {
        let existing = vec![peace_control_core::preset_store::StoredPreset {
            name: "Untitled 1".to_string(),
            path: PathBuf::new(),
            description: String::new(),
        }];
        assert_eq!(next_untitled(&existing), "Untitled 2");
    }

    #[test]
    fn status_opacity_fades_after_six_seconds() {
        let mut status = Status::new("hi", StatusKind::Success);
        assert!(status.opacity() > 0.99);
        status.at = Instant::now() - std::time::Duration::from_secs(10);
        assert!(status.opacity() < 0.01);
    }
}
