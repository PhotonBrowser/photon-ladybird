use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, bail};
use chrono::Local;

use crate::output::BuildDisplay;
use crate::process::{run_inherited, run_logged};
use crate::ui;

pub fn build(repository: &Path, preset: &str, verbose: bool, target: Option<&str>) -> Result<i32> {
    crate::patches::ensure_materialized(repository)?;
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
    crate::patches::ensure_materialized(repository)?;
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

pub fn run_dev(repository: &Path, no_build: bool, verbose: bool, application_args: &[OsString]) -> Result<i32> {
    crate::patches::ensure_materialized(repository)?;
    if photon_binary_needs_build(repository)? {
        if no_build {
            bail!("Photon must be built for dev mode; run `./photon build` first or omit `--no-build`");
        }
        ui::step("Building Photon once for the dev session...");
        let code = build(repository, "Release", verbose, Some("Photon"))?;
        if code != 0 {
            return Ok(code);
        }
    }

    let web_ui_directory = repository.join("Photon/WebUI");
    let mut vite_command = Command::new("npm");
    vite_command.args(["run", "dev"]).current_dir(&web_ui_directory);
    ui::header("Photon", Some("Run · Web UI dev mode"));
    ui::step("Starting Vite with hot reload...");
    let mut vite = crate::process::start_background(&mut vite_command)
        .context("failed to start Vite; run `npm install` in Photon/WebUI first")?;

    if let Err(error) = wait_for_vite(&mut vite) {
        crate::process::stop_background(&mut vite)?;
        return Err(error);
    }
    ui::ok("Vite ready", "http://127.0.0.1:5173 (frontend edits hot reload)");

    let mut command = ladybird_command(repository);
    command.args(["run", "--preset", "Release", "--no-build", "Photon"]);
    command.env("PHOTON_WEBUI_DEV_SERVER", "http://127.0.0.1:5173/");
    command.args(application_args);
    let result = run_inherited(&mut command);
    crate::process::stop_background(&mut vite)?;
    result
}

fn wait_for_vite(vite: &mut std::process::Child) -> Result<()> {
    let address = SocketAddr::from(([127, 0, 0, 1], 5173));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = vite.try_wait()? {
            bail!("Vite dev server exited before becoming ready (status {status})");
        }
        if vite_serves_photon(address) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for Vite at http://127.0.0.1:5173");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn vite_serves_photon(address: SocketAddr) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(150)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let request = b"GET / HTTP/1.1\r\nHost: 127.0.0.1:5173\r\nConnection: close\r\n\r\n";
    if stream.write_all(request).is_err() {
        return false;
    }
    let mut response = Vec::new();
    if stream.read_to_end(&mut response).is_err() {
        return false;
    }
    let response = String::from_utf8_lossy(&response);
    response.contains("200 OK") && response.contains("Photon Chrome") && response.contains("/@vite/client")
}

fn photon_binary_needs_build(repository: &Path) -> Result<bool> {
    let executable = if cfg!(windows) { "Photon.exe" } else { "Photon" };
    let binary = repository.join("Build/release/bin").join(executable);
    let Ok(binary_metadata) = fs::metadata(&binary) else {
        return Ok(true);
    };
    let built_at = binary_metadata
        .modified()
        .with_context(|| format!("failed to read {} timestamp", binary.display()))?;
    if fs::metadata(repository.join("Photon/CMakeLists.txt"))?.modified()? > built_at {
        return Ok(true);
    }
    for directory in ["Photon/App", "Photon/Bridge", "Photon/Rust"] {
        if directory_has_newer_native_source(&repository.join(directory), built_at)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn directory_has_newer_native_source(directory: &Path, built_at: SystemTime) -> Result<bool> {
    for entry in fs::read_dir(directory).with_context(|| format!("failed to read {}", directory.display()))? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            if entry.file_name() == "target" {
                continue;
            }
            if directory_has_newer_native_source(&path, built_at)? {
                return Ok(true);
            }
            continue;
        }
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("cpp" | "h" | "rs" | "toml" | "lock")
        ) && metadata.modified()? > built_at
        {
            return Ok(true);
        }
    }
    Ok(false)
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
