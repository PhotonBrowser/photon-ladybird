use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::Result;
use chrono::Local;

use crate::output::BuildDisplay;
use crate::process::{run_inherited, run_logged};
use crate::ui;

pub fn build(repository: &Path, preset: &str, verbose: bool, target: Option<&str>) -> Result<i32> {
    let mut command = ladybird_command(repository);
    command.args(["build", "--preset", preset]);
    if let Some(target) = target {
        command.arg(target);
    }

    run_build_command(repository, command, preset, verbose, target)
}

pub fn run(
    repository: &Path,
    preset: &str,
    no_build: bool,
    verbose: bool,
    ui: Option<&str>,
    application_args: &[OsString],
) -> Result<i32> {
    if !no_build {
        let code = build(repository, preset, verbose, Some("Photon"))?;
        if code != 0 {
            return Ok(code);
        }
        ui::step("Starting Photon...");
    } else {
        ui::header("Photon", Some("Run · Release · Qt"));
        ui::step("Starting Photon without building...");
    }

    let mut command = ladybird_command(repository);
    command.args(["run", "--preset", preset, "--no-build", "Photon"]);
    if let Some(ui) = ui {
        command.env("PHOTON_UI", ui);
    }
    command.args(application_args);
    run_inherited(&mut command)
}

pub fn clean(repository: &Path, preset: &str) -> Result<i32> {
    let build_directory = repository.join("Build").join(preset_directory(preset));
    remove_path(&build_directory.join("Photon"))?;
    remove_path(&build_directory.join("bin/Photon"))?;
    ui::header("Photon", Some("Clean"));
    ui::ok("Removed", build_directory.display());
    ui::hint("Ladybird dependencies and shared Cargo artifacts were preserved.");
    Ok(0)
}

pub fn test(repository: &Path, preset: &str, pattern: Option<&str>, verbose: bool) -> Result<i32> {
    let mut command = ladybird_command(repository);
    command.args(["test", "--preset", preset]);
    if let Some(pattern) = pattern {
        command.arg(pattern);
    }

    let log_path = new_log_path(repository);
    let started = Instant::now();
    let display_log_path = log_path.strip_prefix(repository).unwrap_or(&log_path);
    let mut display = BuildDisplay::new("Testing Photon", preset, verbose, display_log_path);
    let outcome = run_logged(&mut command, &log_path, verbose, |line| {
        display.observe(line);
    })?;
    display.finish(&outcome, started.elapsed(), None);
    Ok(outcome.code)
}

fn run_build_command(
    repository: &Path,
    mut command: Command,
    preset: &str,
    verbose: bool,
    target: Option<&str>,
) -> Result<i32> {
    command.env("NINJA_STATUS", "[PHOTON %f/%t] ");
    command.env_remove("CLAUDECODE");
    command.env_remove("CODEX_SANDBOX");

    let log_path = new_log_path(repository);
    let started = Instant::now();
    let display_log_path = log_path.strip_prefix(repository).unwrap_or(&log_path);
    let mut display = BuildDisplay::new("Building Photon", preset, verbose, display_log_path);
    let outcome = run_logged(&mut command, &log_path, verbose, |line| {
        display.observe(line);
    })?;
    display.finish(&outcome, started.elapsed(), target);
    Ok(outcome.code)
}

fn ladybird_command(repository: &Path) -> Command {
    let mut command = Command::new("python3");
    command.arg(repository.join("Meta/ladybird.py")).current_dir(repository);
    command
}

fn preset_directory(preset: &str) -> String {
    preset.to_ascii_lowercase().replace("all_debug", "alldebug")
}

fn remove_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn new_log_path(repository: &Path) -> PathBuf {
    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
    repository.join("Build/logs").join(format!("photon-{timestamp}.log"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_uses_the_required_directory_and_prefix() {
        let root = Path::new("/checkout");
        let path = new_log_path(root);
        let relative = path.strip_prefix(root).expect("path should be under root");

        assert_eq!(relative.parent(), Some(Path::new("Build/logs")));
        assert!(
            relative
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("photon-"))
        );
    }
}
