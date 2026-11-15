//! Command line companion for PeaceControl.
//!
//! The GUI is the main product, but every engine capability is scriptable:
//! rendering, validating, importing, exporting and applying configurations.

use peace_control_core::model::Preset;
use peace_control_core::paths::AppPaths;
use peace_control_core::render::{render_config, RenderOptions};
use peace_control_core::{
    audacity, autoeq, peace_ini, preset_store::PresetStore, settings::Settings,
};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub const HELP: &str = "\
PeaceControl command line companion

USAGE:
    peacecontrol-cli <command> [arguments]

COMMANDS:
    render <preset> [--output <file>] [--graphic-eq]
        Write the Equalizer APO configuration for a preset (--output - for stdout).

    validate <preset>
        Check that a preset or .peace file can be read, and summarise it.

    import <file> [--output <file>]
        Convert an Equalizer APO config, REW export, AutoEQ curve or CSV table.

    apply <preset> --target <config.txt>
        Write the configuration where Equalizer APO reads it.

    presets
        List the built in configurations.

    library <directory>
        Write every built in configuration as a .peace file.

    export-audacity <preset> [--output <file>] [--speaker <index>]
        Export the current response as an Audacity equalizer text file.

    doctor
        Show detected Equalizer APO folders, data folders and versions.

    help
        Show this text.
";

/// Runs one command and returns its textual output.
pub fn execute(args: &[String], stdout: &mut String) -> Result<(), String> {
    let (command, rest) = match args.split_first() {
        Some((command, rest)) => (command.as_str(), rest),
        None => {
            stdout.push_str(HELP);
            return Ok(());
        }
    };

    match command {
        "help" | "--help" | "-h" => {
            stdout.push_str(HELP);
            Ok(())
        }
        "render" => command_render(rest, stdout),
        "validate" => command_validate(rest, stdout),
        "import" => command_import(rest, stdout),
        "apply" => command_apply(rest, stdout),
        "presets" => command_presets(stdout),
        "library" => command_library(rest, stdout),
        "export-audacity" => command_export_audacity(rest, stdout),
        "doctor" => command_doctor(stdout),
        other => Err(format!("unknown command '{other}'. Run 'help'.")),
    }
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1).cloned())
}

fn positional(args: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    let mut skip = false;
    for arg in args {
        if skip {
            skip = false;
            continue;
        }
        if arg.starts_with("--") {
            if arg != "--graphic-eq" {
                skip = true;
            }
            continue;
        }
        values.push(arg.clone());
    }
    values
}

/// Loads a preset from JSON, `.peace` or a plain configuration file.
pub fn load_preset(path: &Path) -> Result<Preset, String> {
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    if extension == "json" {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let mut preset: Preset = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        if preset.name.trim().is_empty() {
            if let Some(stem) = path.file_stem() {
                preset.name = stem.to_string_lossy().to_string();
            }
        }
        return Ok(preset);
    }

    if extension == "peace" {
        return peace_ini::load_preset_file(path).map_err(|error| error.to_string());
    }

    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    if let Ok(preset) = peace_ini::parse_preset(&text) {
        if !preset.speakers.is_empty() {
            let mut preset = preset;
            if let Some(stem) = path.file_stem() {
                preset.name = stem.to_string_lossy().to_string();
            }
            return Ok(preset);
        }
    }
    if let Some(report) = peace_control_core::parse::parse_config(&text) {
        return Ok(report.preset);
    }
    Err(format!(
        "{} is not a readable configuration",
        path.display()
    ))
}

fn write_out(target: Option<String>, text: &str, stdout: &mut String) -> Result<(), String> {
    match target.as_deref() {
        None | Some("-") => {
            stdout.push_str(text);
            Ok(())
        }
        Some(path) => {
            let path = PathBuf::from(path);
            peace_control_core::fsutil::write_atomic(&path, text)
                .map_err(|error| error.to_string())?;
            let _ = writeln!(stdout, "wrote {}", path.display());
            Ok(())
        }
    }
}

fn command_render(args: &[String], stdout: &mut String) -> Result<(), String> {
    let files = positional(args);
    let path = files
        .first()
        .ok_or("usage: render <preset> [--output <file>]")?;
    let preset = load_preset(Path::new(path))?;
    let options = RenderOptions {
        graphic_eq: preset.effects.graphic_eq || args.iter().any(|arg| arg == "--graphic-eq"),
        ..Default::default()
    };
    let text = render_config(&preset, &options);
    write_out(flag_value(args, "--output"), &text, stdout)
}

fn command_validate(args: &[String], stdout: &mut String) -> Result<(), String> {
    let files = positional(args);
    let path = files.first().ok_or("usage: validate <preset>")?;
    let preset = load_preset(Path::new(path))?;
    let text = render_config(&preset, &RenderOptions::from_preset(&preset));
    let filters = text.matches("Filter: ON").count();
    let _ = writeln!(stdout, "ok: {}", preset.name);
    let _ = writeln!(stdout, "  speakers : {}", preset.speakers.len());
    let _ = writeln!(stdout, "  bands    : {}", preset.band_count());
    let _ = writeln!(stdout, "  filters  : {filters}");
    let _ = writeln!(stdout, "  commands : {}", preset.commands_effective().len());
    let _ = writeln!(
        stdout,
        "  effects  : {} active",
        preset.effects.active_count()
    );
    let _ = writeln!(stdout, "  neutral  : {}", preset.is_neutral());
    Ok(())
}

fn command_import(args: &[String], stdout: &mut String) -> Result<(), String> {
    let files = positional(args);
    let path = files
        .first()
        .ok_or("usage: import <file> [--output <file>]")?;
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;

    let preset = if let Some(report) = peace_control_core::parse::parse_config(&text) {
        let mut preset = report.preset;
        preset.name = Path::new(path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Imported".to_string());
        preset
    } else {
        let result = autoeq::import_text(&text, 31).map_err(|error| error.to_string())?;
        let mut preset = Preset::with_bands(1);
        preset.name = Path::new(path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Imported".to_string());
        preset.speakers[0].bands = result.bands;
        preset
    };

    let json = serde_json::to_string_pretty(&preset).map_err(|error| error.to_string())?;
    write_out(flag_value(args, "--output"), &format!("{json}\n"), stdout)
}

fn command_apply(args: &[String], stdout: &mut String) -> Result<(), String> {
    let files = positional(args);
    let path = files
        .first()
        .ok_or("usage: apply <preset> --target <config.txt>")?;
    let target = flag_value(args, "--target").ok_or("apply needs --target <config.txt>")?;
    let preset = load_preset(Path::new(path))?;
    let target = PathBuf::from(target);
    let text = render_config(&preset, &RenderOptions::from_preset(&preset));
    peace_control_core::fsutil::write_atomic(&target, &text).map_err(|error| error.to_string())?;
    let _ = writeln!(stdout, "applied '{}' to {}", preset.name, target.display());
    Ok(())
}

fn command_presets(stdout: &mut String) -> Result<(), String> {
    for preset in peace_control_core::library::builtin_presets() {
        let _ = writeln!(
            stdout,
            "{:<28} {:>2} bands  {}",
            preset.name,
            preset.band_count(),
            preset.description
        );
    }
    Ok(())
}

fn command_library(args: &[String], stdout: &mut String) -> Result<(), String> {
    let files = positional(args);
    let dir = files.first().ok_or("usage: library <directory>")?;
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let mut written = 0;
    for preset in peace_control_core::library::builtin_presets() {
        let path = dir.join(format!("{}.peace", preset.name));
        peace_ini::save_preset_file(&path, &preset).map_err(|error| error.to_string())?;
        written += 1;
    }
    let _ = writeln!(stdout, "wrote {written} presets to {}", dir.display());
    Ok(())
}

fn command_export_audacity(args: &[String], stdout: &mut String) -> Result<(), String> {
    let files = positional(args);
    let path = files.first().ok_or("usage: export-audacity <preset>")?;
    let preset = load_preset(Path::new(path))?;
    let speaker: usize = flag_value(args, "--speaker")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let text = audacity::to_audacity_txt(&preset, speaker);
    write_out(flag_value(args, "--output"), &text, stdout)
}

fn command_doctor(stdout: &mut String) -> Result<(), String> {
    let paths = AppPaths::discover();
    let settings = Settings::load(&paths.settings_file());
    let apo = peace_control_core::paths::detect_equalizer_apo(
        settings
            .equalizer_apo_dir
            .as_ref()
            .map(PathBuf::from)
            .as_deref(),
    );

    let _ = writeln!(stdout, "PeaceControl {}", peace_control_core::VERSION);
    let _ = writeln!(stdout, "data folder     : {}", paths.root().display());
    let _ = writeln!(stdout, "portable mode   : {}", paths.is_portable());
    let _ = writeln!(
        stdout,
        "presets         : {}",
        paths.presets_dir().display()
    );
    let _ = writeln!(
        stdout,
        "logs            : {}",
        peace_control_core::logging::log_file(&paths.logs_dir()).display()
    );
    match &apo {
        Some(dir) => {
            let _ = writeln!(stdout, "Equalizer APO   : {}", dir.display());
            let config = peace_control_core::paths::config_dir(dir);
            let targets = peace_control_core::paths::config_targets(&config);
            let _ = writeln!(stdout, "config targets  : {}", targets.len());
            for target in targets {
                let _ = writeln!(stdout, "  - {}", target.display());
            }
        }
        None => {
            let _ = writeln!(stdout, "Equalizer APO   : not detected");
        }
    }
    let store = PresetStore::new(paths.presets_dir());
    let _ = writeln!(stdout, "stored presets  : {}", store.list().len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pc-cli-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join(name)
    }

    fn write_preset() -> PathBuf {
        let path = temp("preset.json");
        let mut preset = Preset::default();
        preset.name = "CLI Test".to_string();
        preset.preamp_db = -3.0;
        preset.speakers[0].bands[0].gain_db = 4.0;
        std::fs::write(&path, serde_json::to_string_pretty(&preset).unwrap()).unwrap();
        path
    }

    fn run(args: &[&str]) -> Result<String, String> {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let mut out = String::new();
        execute(&args, &mut out)?;
        Ok(out)
    }

    #[test]
    fn help_is_available() {
        let output = run(&["help"]).unwrap();
        assert!(output.contains("USAGE"));
        assert!(output.contains("render"));
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(run(&["nonsense"]).is_err());
    }

    #[test]
    fn render_outputs_a_configuration() {
        let path = write_preset();
        let output = run(&["render", path.to_str().unwrap()]).unwrap();
        assert!(output.contains("Preamp: -3 dB"));
        assert!(output.contains("Filter: ON PK"));
    }

    #[test]
    fn validate_summarises_the_preset() {
        let path = write_preset();
        let output = run(&["validate", path.to_str().unwrap()]).unwrap();
        assert!(output.contains("ok: CLI Test"));
        assert!(output.contains("speakers : 1"));
        assert!(output.contains("filters  : 1"));
    }

    #[test]
    fn rendering_to_a_file_writes_it() {
        let path = write_preset();
        let out = temp("config.txt");
        run(&[
            "render",
            path.to_str().unwrap(),
            "--output",
            out.to_str().unwrap(),
        ])
        .unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("Channel: all"));
    }

    #[test]
    fn apply_writes_the_target_file() {
        let path = write_preset();
        let target = temp("apply-target.txt");
        run(&[
            "apply",
            path.to_str().unwrap(),
            "--target",
            target.to_str().unwrap(),
        ])
        .unwrap();
        assert!(std::fs::read_to_string(&target).unwrap().contains("Preamp"));
    }

    #[test]
    fn apply_requires_a_target() {
        let path = write_preset();
        assert!(run(&["apply", path.to_str().unwrap()]).is_err());
    }

    #[test]
    fn presets_are_listed() {
        let output = run(&["presets"]).unwrap();
        assert!(output.contains("Studio Flat 13"));
        assert!(output.contains("Third Octave 31"));
    }

    #[test]
    fn library_export_writes_peace_files() {
        let dir = std::env::temp_dir().join(format!("pc-cli-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        run(&["library", dir.to_str().unwrap()]).unwrap();
        let count = std::fs::read_dir(&dir).unwrap().count();
        assert!(count >= 8);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn doctor_reports_paths() {
        let output = run(&["doctor"]).unwrap();
        assert!(output.contains("PeaceControl"));
        assert!(output.contains("data folder"));
    }

    #[test]
    fn importing_a_config_produces_json() {
        let path = temp("import.txt");
        std::fs::write(
            &path,
            "Preamp: -2 dB\nChannel: L R\nFilter: ON PK Fc 100 Hz Gain 3 dB Q 1\n",
        )
        .unwrap();
        let output = run(&["import", path.to_str().unwrap()]).unwrap();
        assert!(output.contains("\"name\""));
        assert!(output.contains("\"speakers\""));
    }

    #[test]
    fn peace_files_are_readable() {
        let dir = std::env::temp_dir().join(format!("pc-cli-peace-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("Roundtrip.peace");
        let mut preset = Preset::default();
        preset.name = "Roundtrip".to_string();
        preset.speakers[0].bands[0].gain_db = 5.0;
        peace_ini::save_preset_file(&path, &preset).unwrap();
        let output = run(&["validate", path.to_str().unwrap()]).unwrap();
        assert!(output.contains("ok: Roundtrip"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_files_report_errors() {
        assert!(run(&["validate", "C:/definitely/missing.json"]).is_err());
    }

    #[test]
    fn audacity_export_has_rows() {
        let path = write_preset();
        let output = run(&["export-audacity", path.to_str().unwrap()]).unwrap();
        assert!(output.lines().count() > 100);
    }
}
