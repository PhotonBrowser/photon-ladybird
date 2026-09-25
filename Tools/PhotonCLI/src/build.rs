// SPDX-License-Identifier: GPL-3.0-only
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, bail};
use chrono::Local;

use crate::output::BuildDisplay;
use crate::process::{run_inherited, run_logged};
use crate::ui;

pub fn build(repository: &Path, preset: &str, verbose: bool, stats: bool, target: Option<&str>) -> Result<i32> {
    let total_started = Instant::now();
    let source = crate::engine::ensure_build_tree(repository)?;
    let ui_started = Instant::now();
    ensure_webui_production_build(repository, &source)?;
    let stats = stats.then_some(BuildStatsRequest {
        total_started,
        ui_elapsed: ui_started.elapsed(),
    });
    build_native(repository, &source, preset, verbose, target, stats)
}

#[derive(Clone, Copy)]
struct BuildStatsRequest {
    total_started: Instant,
    ui_elapsed: Duration,
}

fn build_native(
    repository: &Path,
    source: &Path,
    preset: &str,
    verbose: bool,
    target: Option<&str>,
    stats: Option<BuildStatsRequest>,
) -> Result<i32> {
    ensure_configured_for_source(source, preset)?;
    let mut command = ladybird_command(source);
    command.args(["build", "--preset", preset]);
    if let Some(target) = target {
        command.arg(target);
    }

    run_build_command(repository, source, command, preset, verbose, target, stats)
}

/// Ladybird's build helper reuses an existing CMake generator without checking
/// which source tree created it. Photon can build from either generated tree,
/// so invalidate only the generator marker when the shared cache points at a
/// different source. Remove only CMake's generated configuration; compiled
/// objects and shared dependency caches remain available for regeneration.
fn ensure_configured_for_source(source: &Path, preset: &str) -> Result<()> {
    let build_directory = source.join("Build").join(preset_directory(preset));
    let cache = build_directory.join("CMakeCache.txt");
    let Ok(contents) = fs::read_to_string(&cache) else {
        return Ok(());
    };
    let configured_source = contents
        .lines()
        .find_map(|line| line.strip_prefix("CMAKE_HOME_DIRECTORY:INTERNAL="));
    let configured_build = contents
        .lines()
        .find_map(|line| line.strip_prefix("CMAKE_CACHEFILE_DIR:INTERNAL="));
    let expected_source = source.canonicalize()?;
    let expected_build = build_directory.canonicalize()?;
    if configured_source.is_some_and(|configured| Path::new(configured) == expected_source)
        && configured_build.is_some_and(|configured| Path::new(configured) == expected_build)
    {
        return Ok(());
    }
    for marker in ["build.ninja", "ladybird.sln"] {
        let marker = build_directory.join(marker);
        if marker.exists() {
            fs::remove_file(&marker).with_context(|| format!("failed to invalidate {}", marker.display()))?;
        }
    }
    remove_path(&cache)?;
    remove_path(&build_directory.join("CMakeFiles"))?;
    Ok(())
}

pub fn run(
    repository: &Path,
    preset: &str,
    no_build: bool,
    verbose: bool,
    application_args: &[OsString],
) -> Result<i32> {
    let source = crate::engine::ensure_build_tree(repository)?;
    if !no_build {
        let code = build(repository, preset, verbose, false, Some("Photon"))?;
        if code != 0 {
            return Ok(code);
        }
        ui::step("Starting Photon...");
    } else {
        ui::header("Photon", Some("Run · Release · Qt"));
        if webui_bundle_is_stale(&source).unwrap_or(false) {
            ui::hint("Web UI bundle is stale; omit `--no-build` so `./photon run` rebuilds the Vite app.");
        }
        ui::step("Starting Photon without building...");
    }

    let mut command = ladybird_command(&source);
    command.args(["run", "--preset", preset, "--no-build", "Photon"]);
    command.args(application_args);
    run_inherited(&mut command)
}

pub fn run_dev(repository: &Path, no_build: bool, verbose: bool, application_args: &[OsString]) -> Result<i32> {
    let source = crate::engine::ensure_build_tree(repository)?;
    ensure_webui_dependencies(repository)?;
    let mut sources = dev_source_snapshot(repository, &source)?;
    if photon_binary_needs_build(&source, &sources)? {
        if no_build {
            bail!("Photon must be built for dev mode; run `./photon build` first or omit `--no-build`");
        }
        ui::step("Building Photon for dev mode...");
        let code = build_native(repository, &source, "Release", verbose, Some("Photon"), None)?;
        if code != 0 {
            return Ok(code);
        }
    }

    ui::header("Photon", Some("Run · Web UI dev mode"));
    let mut vite = start_vite(repository)?;
    if let Err(error) = wait_for_vite(&mut vite) {
        crate::process::stop_background(&mut vite)?;
        if crate::process::interrupt_requested() {
            return Ok(130);
        }
        return Err(error);
    }
    ui::ok("Vite ready", "http://127.0.0.1:5173 (frontend edits hot reload)");
    let mut photon = match start_dev_photon(&source, application_args) {
        Ok(photon) => Some(photon),
        Err(error) => {
            crate::process::stop_background(&mut vite)?;
            return Err(error);
        }
    };

    loop {
        thread::sleep(Duration::from_millis(500));

        if crate::process::interrupt_requested() {
            if let Some(photon) = &mut photon {
                crate::process::stop_managed(photon)?;
            }
            crate::process::stop_background(&mut vite)?;
            return Ok(130);
        }

        if let Some(photon) = &mut photon {
            if let Some(code) = crate::process::try_wait_managed(photon)? {
                crate::process::stop_background(&mut vite)?;
                return Ok(code);
            }
        }
        if let Some(status) = vite.try_wait()? {
            if let Some(photon) = &mut photon {
                crate::process::stop_managed(photon)?;
            }
            bail!("Vite dev server exited unexpectedly (status {status})");
        }

        let next_sources = match dev_source_snapshot(repository, &source) {
            Ok(sources) => sources,
            Err(error) => {
                if let Some(photon) = &mut photon {
                    crate::process::stop_managed(photon)?;
                }
                crate::process::stop_background(&mut vite)?;
                return Err(error);
            }
        };
        if next_sources == sources {
            continue;
        }

        ui::step("Source or build configuration changed; restarting Photon...");
        if let Some(photon) = &mut photon {
            crate::process::stop_managed(photon)?;
        }
        photon = None;
        crate::process::stop_background(&mut vite)?;
        thread::sleep(Duration::from_millis(300));

        let source = crate::engine::ensure_build_tree(repository)?;
        sources = dev_source_snapshot(repository, &source)?;
        ensure_webui_dependencies(repository)?;
        let code = build_native(repository, &source, "Release", verbose, Some("Photon"), None)?;
        if code == 130 || crate::process::interrupt_requested() {
            return Ok(130);
        }
        let mut vite_result = start_vite(repository)?;
        if let Err(error) = wait_for_vite(&mut vite_result) {
            crate::process::stop_background(&mut vite_result)?;
            if crate::process::interrupt_requested() {
                return Ok(130);
            }
            return Err(error);
        }
        vite = vite_result;
        if code == 0 {
            photon = match start_dev_photon(&source, application_args) {
                Ok(photon) => Some(photon),
                Err(error) => {
                    crate::process::stop_background(&mut vite)?;
                    return Err(error);
                }
            };
            ui::ok("Photon restarted", "Rust, C++, and build configuration changes applied");
        } else {
            ui::hint("Photon build failed; save another source change to retry.");
        }
    }
}

fn start_vite(repository: &Path) -> Result<Child> {
    let web_ui_directory = webui_directory(repository);
    let mut command = Command::new("npm");
    command.args(["run", "dev"]).current_dir(web_ui_directory);
    ui::step("Starting Vite with hot reload...");
    crate::process::start_background(&mut command)
        .context("failed to start Vite; run `npm install` in Photon/WebUI first")
}

fn start_dev_photon(source: &Path, application_args: &[OsString]) -> Result<Child> {
    let mut command = ladybird_command(source);
    command.args(["run", "--preset", "Release", "--no-build", "Photon"]);
    command.env("PHOTON_WEBUI_DEV_SERVER", "http://127.0.0.1:5173/");
    command.args(application_args);
    crate::process::start_managed(&mut command)
}

fn webui_directory(source: &Path) -> PathBuf {
    source.join("Photon/WebUI")
}

/// Build the Vite production bundle so plain `./photon run` renders the
/// bundled chrome without a dev server. Skipped only when the bundle is
/// already newer than every WebUI source file.
pub fn ensure_webui_production_build(repository: &Path, source: &Path) -> Result<()> {
    ensure_webui_dependencies(repository)?;
    if webui_bundle_is_stale(repository)? {
        let web_ui_directory = webui_directory(repository);
        ui::step("Building Photon Web UI (Vite production bundle)...");
        let mut command = Command::new("npm");
        command.args(["run", "build"]).current_dir(&web_ui_directory);
        let code =
            crate::process::run_inherited(&mut command).context("failed to run `npm run build` in Photon/WebUI")?;
        if code != 0 {
            bail!("`npm run build` in Photon/WebUI failed with exit code {code}");
        }
    }

    let generated_bundle = webui_directory(repository).join("dist/index.html");
    let build_bundle = webui_directory(source).join("dist/index.html");
    fs::create_dir_all(build_bundle.parent().expect("bundle has parent directory"))?;
    fs::copy(&generated_bundle, &build_bundle)
        .with_context(|| format!("failed to stage generated Web UI bundle at {}", build_bundle.display()))?;
    ui::ok("Web UI ready", "Photon/WebUI/dist/index.html");
    Ok(())
}

/// Ensure `node_modules` exists so Vite build/dev commands can run.
/// Runs `npm ci` when a lockfile is present, otherwise `npm install`.
fn ensure_webui_dependencies(source: &Path) -> Result<()> {
    let web_ui_directory = webui_directory(source);
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
fn webui_bundle_is_stale(source: &Path) -> Result<bool> {
    let web_ui_directory = webui_directory(source);
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

fn photon_binary_needs_build(source: &Path, sources: &HashMap<PathBuf, SourceFileState>) -> Result<bool> {
    let executable = if cfg!(windows) { "Photon.exe" } else { "Photon" };
    let binary = source.join("Build/release/bin").join(executable);
    let Ok(binary_metadata) = fs::metadata(&binary) else {
        return Ok(true);
    };
    let built_at = binary_metadata
        .modified()
        .with_context(|| format!("failed to read {} timestamp", binary.display()))?;
    Ok(sources.values().any(|source| source.modified > built_at))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceFileState {
    modified: SystemTime,
    size: u64,
}

fn dev_source_snapshot(repository: &Path, source: &Path) -> Result<HashMap<PathBuf, SourceFileState>> {
    let mut sources = HashMap::new();
    collect_dev_sources(source, &mut sources)?;
    collect_dev_sources(&repository.join("Photon"), &mut sources)?;
    collect_dev_sources(&repository.join("Patches"), &mut sources)?;
    Ok(sources)
}

fn collect_dev_sources(directory: &Path, sources: &mut HashMap<PathBuf, SourceFileState>) -> Result<()> {
    for entry in fs::read_dir(directory).with_context(|| format!("failed to read {}", directory.display()))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if should_skip_dev_directory(&entry.file_name()) {
                continue;
            }
            collect_dev_sources(&path, sources)?;
            continue;
        }
        if !is_dev_build_input(&path) {
            continue;
        }
        let metadata = entry.metadata()?;
        sources.insert(
            path,
            SourceFileState {
                modified: metadata.modified()?,
                size: metadata.len(),
            },
        );
    }
    Ok(())
}

fn should_skip_dev_directory(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(".git" | ".cache" | ".venv" | "Build" | "Tools" | "dist" | "node_modules" | "target" | "vcpkg")
    )
}

fn is_dev_build_input(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(
            "c" | "cc"
                | "cpp"
                | "cxx"
                | "h"
                | "hh"
                | "hpp"
                | "hxx"
                | "inl"
                | "ipp"
                | "inc"
                | "rs"
                | "toml"
                | "lock"
                | "cmake"
                | "py"
                | "sh"
                | "json"
                | "yaml"
                | "yml"
                | "ini"
                | "cfg"
                | "ninja"
                | "bzl"
                | "patch"
                | "qrc"
                | "ui"
                | "xml"
                | "pro"
                | "pri"
        )
    ) || matches!(
        file_name,
        "CMakeLists.txt"
            | "Makefile"
            | "makefile"
            | "meson.build"
            | "BUILD"
            | "BUILD.bazel"
            | "vite.config.ts"
            | "tsconfig.json"
            | "tsconfig.app.json"
            | "tsconfig.node.json"
            | "CMakePresets.json"
            | "CMakeUserPresets.json"
    ) || file_name.ends_with(".cmake.in")
        || file_name.ends_with(".h.in")
        || file_name.ends_with(".hpp.in")
}

pub fn clean(repository: &Path, preset: &str) -> Result<i32> {
    let source = crate::engine::ensure_build_tree(repository)?;
    let build_directory = source.join("Build").join(preset_directory(preset));
    remove_path(&build_directory.join("Photon"))?;
    remove_path(&build_directory.join("bin/Photon"))?;
    ui::header("Photon", Some("Clean"));
    ui::ok("Removed", build_directory.display());
    ui::hint("Ladybird dependencies and shared Cargo artifacts were preserved.");
    Ok(0)
}

pub fn test(repository: &Path, preset: &str, pattern: Option<&str>, verbose: bool) -> Result<i32> {
    let source = crate::engine::ensure_build_tree(repository)?;
    let mut command = ladybird_command(&source);
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
    source: &Path,
    mut command: Command,
    preset: &str,
    verbose: bool,
    target: Option<&str>,
    stats: Option<BuildStatsRequest>,
) -> Result<i32> {
    command.env("NINJA_STATUS", "[PHOTON %f/%t] ");
    command.env_remove("CLAUDECODE");
    command.env_remove("CODEX_SANDBOX");

    let log_path = new_log_path(repository);
    let started = Instant::now();
    let display_log_path = log_path.strip_prefix(repository).unwrap_or(&log_path);
    let mut display = BuildDisplay::new("Building Photon", preset, verbose, display_log_path);
    let build_directory = source.join("Build").join(preset_directory(preset));
    let previous_ninja_log = stats
        .map(|_| NinjaLogSnapshot::read(&build_directory.join(".ninja_log")))
        .transpose()?;
    let ccache_before = stats.and_then(|_| ccache_calls());
    let outcome = run_logged(&mut command, &log_path, verbose, |line| {
        display.observe(line);
    })?;
    display.finish(&outcome, started.elapsed(), target);
    if let (Some(stats), Some(previous_ninja_log)) = (stats, previous_ninja_log.as_ref()) {
        if let Err(error) = report_build_stats(&build_directory, stats, previous_ninja_log, ccache_before) {
            eprintln!("Photon build stats unavailable: {error:#}");
        }
    }
    Ok(outcome.code)
}

#[derive(Default)]
struct NinjaBuildStats {
    compile_ms: u64,
    link_ms: u64,
    codegen_ms: u64,
    other_ms: u64,
    slowest_units: Vec<(u64, String)>,
}

#[derive(Clone, Copy, Default)]
struct CcacheCalls {
    hits: u64,
    misses: u64,
}

struct NinjaLogSnapshot {
    contents: String,
}

impl NinjaLogSnapshot {
    fn read(path: &Path) -> Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error).with_context(|| format!("failed to read {}", path.display())),
        };
        Ok(Self { contents })
    }
}

fn report_build_stats(
    build_directory: &Path,
    request: BuildStatsRequest,
    previous_ninja_log: &NinjaLogSnapshot,
    ccache_before: Option<CcacheCalls>,
) -> Result<()> {
    let build_elapsed = request.total_started.elapsed();
    let mut ninja = Command::new("ninja");
    ninja.arg("-C").arg(build_directory).args(["-t", "compdb"]);
    let output = ninja.output().context("failed to run `ninja -t compdb`")?;
    if !output.status.success() {
        bail!(
            "`ninja -t compdb` failed with exit code {}",
            output.status.code().unwrap_or(-1)
        );
    }

    let compile_commands = build_directory.join("compile_commands.json");
    fs::write(&compile_commands, &output.stdout)
        .with_context(|| format!("failed to write {}", compile_commands.display()))?;
    let profile = profile_ninja_log(&build_directory.join(".ninja_log"), &output.stdout, previous_ninja_log)?;

    println!("\nPhoton build: {:.1}s", build_elapsed.as_secs_f64());
    println!("Slowest compilation units");
    for (duration, unit) in &profile.slowest_units {
        println!("  {:>5.1}s  {unit}", *duration as f64 / 1000.0);
    }
    if profile.slowest_units.is_empty() {
        println!("  No compilation units ran");
    }
    println!(
        "\nCompile       {:>5.1}s (summed unit time)",
        profile.compile_ms as f64 / 1000.0
    );
    println!(
        "Link          {:>5.1}s (summed operation time)",
        profile.link_ms as f64 / 1000.0
    );
    println!(
        "Codegen       {:>5.1}s (summed operation time)",
        profile.codegen_ms as f64 / 1000.0
    );
    if profile.other_ms > 0 {
        println!(
            "Other         {:>5.1}s (summed operation time)",
            profile.other_ms as f64 / 1000.0
        );
    }
    println!("Photon UI     {:>5.1}s", request.ui_elapsed.as_secs_f64());
    println!("Compile database: {}", compile_commands.display());

    if let (Some(before), Some(after)) = (ccache_before, ccache_calls()) {
        let hits = after.hits.saturating_sub(before.hits);
        let misses = after.misses.saturating_sub(before.misses);
        let calls = hits + misses;
        if calls > 0 {
            println!(
                "ccache hit rate: {:.0}% ({hits}/{calls} calls)",
                hits as f64 * 100.0 / calls as f64
            );
        }
    }
    Ok(())
}

fn profile_ninja_log(
    log_path: &Path,
    compile_commands: &[u8],
    previous_log: &NinjaLogSnapshot,
) -> Result<NinjaBuildStats> {
    let command_list: serde_json::Value =
        serde_json::from_slice(compile_commands).context("invalid Ninja compilation database")?;
    let mut compile_units = HashMap::new();
    if let Some(commands) = command_list.as_array() {
        for command in commands {
            let (Some(output), Some(file)) = (command["output"].as_str(), command["file"].as_str()) else {
                continue;
            };
            if output.ends_with(".o") || output.ends_with(".obj") {
                compile_units.insert(normalize_ninja_path(output), file.to_owned());
            }
        }
    }

    let log = match fs::read_to_string(log_path) {
        Ok(log) => log,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(NinjaBuildStats::default()),
        Err(error) => return Err(error).with_context(|| format!("failed to read {}", log_path.display())),
    };
    let new_log_entries = if log.starts_with(&previous_log.contents) {
        &log[previous_log.contents.len()..]
    } else {
        // Ninja may compact the log; in that case select records absent from
        // the pre-build snapshot instead of comparing per-process timestamps.
        let previous_entries = previous_log.contents.lines().collect::<std::collections::HashSet<_>>();
        let added_entries = log
            .lines()
            .filter(|line| !previous_entries.contains(line))
            .collect::<Vec<_>>();
        return profile_ninja_entries(&added_entries.join("\n"), &compile_units, log_path);
    };

    profile_ninja_entries(new_log_entries, &compile_units, log_path)
}

fn profile_ninja_entries(
    entries: &str,
    compile_units: &HashMap<String, String>,
    log_path: &Path,
) -> Result<NinjaBuildStats> {
    let mut stats = NinjaBuildStats::default();
    for line in entries.lines().filter(|line| !line.starts_with('#')) {
        let mut fields = line.split('\t');
        let (Some(start), Some(end), Some(_), Some(output)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let (Ok(start), Ok(end)) = (start.parse::<u64>(), end.parse::<u64>()) else {
            continue;
        };
        if end < start {
            continue;
        }

        let duration = end - start;
        let output = normalize_ninja_path(output);
        if let Some(unit) = compile_units.get(&output) {
            stats.compile_ms += duration;
            let source_root = log_path
                .ancestors()
                .nth(3)
                .and_then(|path| path.canonicalize().ok())
                .unwrap_or_else(|| log_path.ancestors().nth(3).unwrap_or(log_path).to_path_buf());
            let unit = Path::new(unit)
                .strip_prefix(&source_root)
                .unwrap_or(Path::new(unit))
                .to_string_lossy()
                .into_owned();
            stats.slowest_units.push((duration, unit));
        } else if is_link_output(&output) {
            stats.link_ms += duration;
        } else if is_codegen_output(&output) {
            stats.codegen_ms += duration;
        } else {
            stats.other_ms += duration;
        }
    }
    stats.slowest_units.sort_by(|left, right| right.0.cmp(&left.0));
    stats.slowest_units.truncate(5);
    Ok(stats)
}

fn normalize_ninja_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn is_link_output(output: &str) -> bool {
    [".a", ".so", ".dylib", ".dll", ".exe"]
        .iter()
        .any(|extension| output.ends_with(extension))
        || output.starts_with("bin/")
        || output.contains("/bin/")
}

fn is_codegen_output(output: &str) -> bool {
    let generated_path = output.to_ascii_lowercase();
    ["generated/", "_generated", "/bindings/", "generate_"]
        .iter()
        .any(|marker| generated_path.contains(marker))
}

fn ccache_calls() -> Option<CcacheCalls> {
    let output = Command::new("ccache").arg("--show-stats").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let report = String::from_utf8_lossy(&output.stdout);
    let mut calls = CcacheCalls::default();
    let (mut found_hits, mut found_misses) = (false, false);
    for line in report.lines() {
        let Some((label, value)) = line.trim().split_once(':') else {
            continue;
        };
        let Some(count) = value
            .trim()
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        match label {
            "Hits" if !found_hits => {
                calls.hits = count;
                found_hits = true;
            }
            "Misses" if !found_misses => {
                calls.misses = count;
                found_misses = true;
            }
            _ => {}
        }
    }
    (found_hits && found_misses).then_some(calls)
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
