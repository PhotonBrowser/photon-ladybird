// SPDX-License-Identifier: GPL-3.0-only
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::process::run_inherited;
use crate::ui;

pub fn format(repository: &Path, check: bool) -> Result<i32> {
    ui::header("Photon", Some(if check { "Format check" } else { "Format" }));

    for manifest in ["Photon/Rust/Cargo.toml", "Tools/PhotonCLI/Cargo.toml"] {
        let mut command = Command::new("cargo");
        command
            .current_dir(repository)
            .args(["fmt", "--manifest-path", manifest]);
        if check {
            command.args(["--", "--check"]);
        }
        ui::step(format!("{} {manifest}", if check { "Checking" } else { "Formatting" }));
        run_step(command)?;
    }
    let cpp_files = photon_cpp_files(repository)?;
    if !cpp_files.is_empty() {
        let mut command = Command::new(clang_format_executable());
        command.current_dir(repository).args(["-style=file"]);
        if check {
            command.args(["--dry-run", "--Werror"]);
        } else {
            command.arg("-i");
        }
        command.args(cpp_files);
        ui::step(if check {
            "Checking Photon C++ formatting"
        } else {
            "Formatting Photon C++"
        });
        run_step(command)?;
    }

    let mut command = Command::new("npm");
    command
        .current_dir(repository.join("Photon/WebUI"))
        .args(["run", if check { "format:check" } else { "format" }]);
    ui::step(if check {
        "Checking WebUI formatting"
    } else {
        "Formatting WebUI"
    });
    run_step(command)?;

    ui::success(if check {
        "All Photon formatting checks passed."
    } else {
        "Photon formatting complete."
    });
    Ok(0)
}

pub fn lint(repository: &Path) -> Result<i32> {
    ui::header("Photon", Some("Lint and type checks"));

    let cpp_files = photon_cpp_files(repository)?;
    if !cpp_files.is_empty() {
        let mut command = Command::new(clang_format_executable());
        command
            .current_dir(repository)
            .args(["-style=file", "--dry-run", "--Werror"])
            .args(cpp_files);
        ui::step("Checking Photon C++ formatting");
        run_step(command)?;
    }

    for manifest in ["Photon/Rust/Cargo.toml", "Tools/PhotonCLI/Cargo.toml"] {
        let mut command = Command::new("cargo");
        command.current_dir(repository).args([
            "clippy",
            "--manifest-path",
            manifest,
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]);
        ui::step(format!("Clippy {manifest}"));
        run_step(command)?;
    }

    for script in ["typecheck", "lint"] {
        let mut command = Command::new("npm");
        command
            .current_dir(repository.join("Photon/WebUI"))
            .args(["run", script]);
        ui::step(format!("WebUI {script}"));
        run_step(command)?;
    }

    ui::success("All Photon linters and type checks passed.");
    Ok(0)
}

pub fn check(repository: &Path) -> Result<i32> {
    if format(repository, true)? != 0 {
        return Ok(1);
    }
    lint(repository)
}

fn run_step(mut command: Command) -> Result<()> {
    let code = run_inherited(&mut command)?;
    if code != 0 {
        bail!("quality check failed with exit code {code}");
    }
    Ok(())
}

fn photon_cpp_files(repository: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .current_dir(repository)
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "Photon/Bridge",
            "Photon/App",
        ])
        .output()
        .context("failed to list Photon C++ source files")?;
    if !output.status.success() {
        bail!("git ls-files failed while locating Photon C++ source files");
    }
    let files = String::from_utf8(output.stdout).context("git returned non-UTF-8 Photon paths")?;
    Ok(files
        .lines()
        .filter(|file| file.ends_with(".cpp") || file.ends_with(".h") || file.ends_with(".mm"))
        .map(PathBuf::from)
        .collect())
}

fn clang_format_executable() -> &'static str {
    if Command::new(if cfg!(windows) {
        "clang-format-21.exe"
    } else {
        "clang-format-21"
    })
    .arg("--version")
    .output()
    .is_ok_and(|output| output.status.success())
    {
        if cfg!(windows) {
            "clang-format-21.exe"
        } else {
            "clang-format-21"
        }
    } else if cfg!(windows) {
        "clang-format.exe"
    } else {
        "clang-format"
    }
}
