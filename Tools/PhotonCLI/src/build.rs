use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::Result;
use chrono::Local;
use console::style;

use crate::output::BuildDisplay;
use crate::process::{run_inherited, run_logged};

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
    application_args: &[OsString],
) -> Result<i32> {
    if !no_build {
        let code = build(repository, preset, verbose, Some("Ladybird"))?;
        if code != 0 {
            return Ok(code);
        }
        println!("{} Starting Photon...", style("→").cyan().bold());
    } else {
        println!("{} Starting Photon without building...", style("→").cyan().bold());
    }

    let mut command = ladybird_command(repository);
    command.args(["run", "--preset", preset, "--no-build", "Ladybird"]);
    command.args(application_args);
    run_inherited(&mut command)
}

pub fn clean(repository: &Path, preset: &str) -> Result<i32> {
    let mut command = ladybird_command(repository);
    command.args(["clean", "--preset", preset]);
    run_inherited(&mut command)
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
