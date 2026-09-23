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
    ensure_webui_production_build(repository)?;
    build_native(repository, preset, verbose, target)
}

fn build_native(repository: &Path, preset: &str, verbose: bool, target: Option<&str>) -> Result<i32> {
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
        if webui_bundle_is_stale(repository).unwrap_or(false) {
            ui::hint("Web UI bundle is stale; omit `--no-build` so `./photon run` rebuilds the Vite app.");
        }
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
    // Dev mode serves the frontend from Vite, so only the native binary is
    // built here. The production bundle is built by plain `./photon run`.
    ensure_webui_dependencies(repository)?;
    if photon_binary_needs_build(repository)? {
        if no_build {
            bail!("Photon must be built for dev mode; run `./photon build` first or omit `--no-build`");
        }
        ui::step("Building Photon once for the dev session...");
        let code = build_native(repository, "Release", verbose, Some("Photon"))?;
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

fn webui_directory(repository: &Path) -> PathBuf {
    repository.join("Photon/WebUI")
}

/// Build the Vite production bundle so plain `./photon run` renders the
/// bundled chrome without a dev server. Skipped only when the bundle is
/// already newer than every WebUI source file.
pub fn ensure_webui_production_build(repository: &Path) -> Result<()> {
    ensure_webui_dependencies(repository)?;
    if !webui_bundle_is_stale(repository)? {
        return Ok(());
    }
    let web_ui_directory = webui_directory(repository);
    ui::step("Building Photon Web UI (Vite production bundle)...");
    let mut command = Command::new("npm");
    command.args(["run", "build"]).current_dir(&web_ui_directory);
    let code = crate::process::run_inherited(&mut command).context("failed to run `npm run build` in Photon/WebUI")?;
    if code != 0 {
        bail!("`npm run build` in Photon/WebUI failed with exit code {code}");
    }
    ui::ok("Web UI ready", "Photon/WebUI/dist/index.html");
    Ok(())
}

/// Ensure `node_modules` exists so Vite build/dev commands can run.
/// Runs `npm ci` when a lockfile is present, otherwise `npm install`.
fn ensure_webui_dependencies(repository: &Path) -> Result<()> {
    let web_ui_directory = webui_directory(repository);
    let node_modules = web_ui_directory.join("node_modules");
    if !needs_npm_install(&web_ui_directory, &node_modules)? {
        return Ok(());
    }
    ui::step("Installing Photon Web UI dependencies...");
    let mut command = Command::new("npm");
    if web_ui_directory.join("package-lock.json").is_file() {
        command.args(["ci"]);
    } else {
        command.args(["install"]);
    }
    command.current_dir(&web_ui_directory);
    let code = crate::process::run_inherited(&mut command)
        .context("failed to install Photon/WebUI dependencies; run `npm install` in Photon/WebUI manually")?;
    if code != 0 {
        bail!("Web UI dependency install failed with exit code {code}");
    }
    Ok(())
}

fn needs_npm_install(web_ui_directory: &Path, node_modules: &Path) -> Result<bool> {
    if !node_modules.is_dir() {
        return Ok(true);
    }
    let node_modules_modified = fs::metadata(node_modules)
        .with_context(|| format!("failed to read {} timestamp", node_modules.display()))?
        .modified()?;
    for manifest in ["package.json", "package-lock.json"] {
        let path = web_ui_directory.join(manifest);
        if path.is_file() && fs::metadata(&path)?.modified()? > node_modules_modified {
            return Ok(true);
        }
    }
    Ok(false)
}

/// True when `dist/index.html` is missing or older than any WebUI source,
/// manifest, or Vite/TS configuration file.
fn webui_bundle_is_stale(repository: &Path) -> Result<bool> {
    let web_ui_directory = webui_directory(repository);
    let bundle = web_ui_directory.join("dist/index.html");
    let Ok(bundle_metadata) = fs::metadata(&bundle) else {
        return Ok(true);
    };
    let built_at = bundle_metadata
        .modified()
        .with_context(|| format!("failed to read {} timestamp", bundle.display()))?;
    Ok(webui_source_is_newer_than(&web_ui_directory, built_at)?
        || webui_source_is_newer_than(&web_ui_directory.join("src"), built_at)?)
}

fn webui_source_is_newer_than(directory: &Path, built_at: SystemTime) -> Result<bool> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(false);
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            if entry.file_name() == "node_modules" || entry.file_name() == "dist" {
                continue;
            }
            if webui_source_is_newer_than(&path, built_at)? {
                return Ok(true);
            }
            continue;
        }
        let watched = matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("html" | "ts" | "tsx" | "js" | "jsx" | "css" | "json" | "svg")
        ) || matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some(
                "package.json"
                    | "package-lock.json"
                    | "vite.config.ts"
                    | "tsconfig.json"
                    | "tsconfig.app.json"
                    | "tsconfig.node.json"
            )
        );
        if watched && metadata.modified()? > built_at {
            return Ok(true);
        }
    }
    Ok(false)
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
    use std::time::Duration;

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

    #[test]
    fn missing_bundle_counts_as_stale() {
        let root = Path::new("/definitely/not/a/photon/checkout");
        assert!(webui_bundle_is_stale(root).expect("missing bundle should report stale"));
    }

    #[test]
    fn newer_source_counts_as_stale() {
        let directory = std::env::temp_dir().join(format!("photon-webui-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("test directory should be created");
        let source = directory.join("probe.tsx");
        fs::write(&source, "export default 1;").expect("test source should be written");
        let older = SystemTime::now() - Duration::from_secs(5);
        assert!(webui_source_is_newer_than(&directory, older).expect("newer source should be stale"));
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn missing_node_modules_requires_install() {
        let web_ui = Path::new("/definitely/not/a/photon/WebUI");
        let node_modules = web_ui.join("node_modules");
        assert!(needs_npm_install(web_ui, &node_modules).expect("missing node_modules needs install"));
    }
}
