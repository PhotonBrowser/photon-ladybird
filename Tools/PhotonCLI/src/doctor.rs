use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use console::style;

const MINIMUM_CMAKE: Version = Version::new(3, 30, 0);
const FALLBACK_MINIMUM_QT: Version = Version::new(6, 9, 0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self { major, minor, patch }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

pub fn run(repository: &Path) -> Result<i32> {
    println!("{}\n", style("Photon development environment").bold());
    let mut blocking_problems = 0;

    blocking_problems += check_tool("Git", "git", &["--version"], None);
    blocking_problems += check_tool("Rust", "rustc", &["--version"], None);
    blocking_problems += check_tool("Cargo", "cargo", &["--version"], None);
    blocking_problems += check_tool("CMake", "cmake", &["--version"], Some(MINIMUM_CMAKE));
    blocking_problems += check_tool("Ninja", "ninja", &["--version"], None);

    let c_compiler = env::var_os("CC").unwrap_or_else(|| "cc".into());
    blocking_problems += check_tool_os("C compiler", &c_compiler, &["--version"], None);
    let cpp_compiler = env::var_os("CXX").unwrap_or_else(|| "c++".into());
    blocking_problems += check_tool_os("C++ compiler", &cpp_compiler, &["--version"], None);

    let minimum_qt = qt_requirement(repository).unwrap_or(FALLBACK_MINIMUM_QT);
    blocking_problems += check_qt(minimum_qt);

    for (label, program) in [
        ("Python", "python3"),
        ("pkg-config", "pkg-config"),
        ("NASM", "nasm"),
        ("Autoconf", "autoconf"),
        ("Automake", "automake"),
        ("Libtool", "libtool"),
        ("Patchelf", "patchelf"),
    ] {
        blocking_problems += check_tool(label, program, &["--version"], None);
    }

    if repository.join("Meta/ladybird.py").is_file() {
        print_ok("Ladybird tools", "found");
    } else {
        print_error("Ladybird tools", "Meta/ladybird.py not found");
        blocking_problems += 1;
    }

    blocking_problems += check_origin_remote(repository);

    report_build_directory(repository, "release");
    report_build_directory(repository, "debug");

    println!();
    if blocking_problems == 0 {
        println!("{}", style("Ready to build Photon.").green().bold());
        Ok(0)
    } else {
        println!(
            "{}",
            style(format!(
                "{blocking_problems} blocking problem(s) found. No packages were changed."
            ))
            .red()
            .bold()
        );
        Ok(1)
    }
}

fn check_tool(label: &str, program: &str, args: &[&str], minimum: Option<Version>) -> i32 {
    check_tool_os(label, OsStr::new(program), args, minimum)
}

fn check_tool_os(label: &str, program: &OsStr, args: &[&str], minimum: Option<Version>) -> i32 {
    let output = Command::new(program).args(args).output();
    let Ok(output) = output else {
        print_error(label, "Not found in PATH");
        return 1;
    };
    if !output.status.success() {
        print_error(label, "Could not determine version");
        return 1;
    }

    let text = combined_output(&output.stdout, &output.stderr);
    let Some(version) = first_version(&text) else {
        print_error(label, "Could not determine version");
        return 1;
    };
    if let Some(required) = minimum {
        if version < required {
            print_version_error(label, required, version);
            return 1;
        }
    }
    print_ok(label, &version.to_string());
    0
}

fn check_qt(required: Version) -> i32 {
    let candidates: [(&str, &[&str]); 3] = [
        ("qtpaths6", &["--qt-version"]),
        ("qmake6", &["-query", "QT_VERSION"]),
        ("pkg-config", &["--modversion", "Qt6Core"]),
    ];
    for (program, args) in candidates {
        let Ok(output) = Command::new(program).args(args).output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let text = combined_output(&output.stdout, &output.stderr);
        let Some(version) = first_version(&text) else {
            continue;
        };
        if version < required {
            print_version_error("Qt", required, version);
            return 1;
        }
        print_ok("Qt", &version.to_string());
        return 0;
    }

    print_error("Qt", &format!("Not found (required: Qt >= {required})"));
    1
}

fn check_origin_remote(repository: &Path) -> i32 {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repository)
        .output();
    let Ok(output) = output else {
        print_error("Origin remote", "could not inspect Git remotes");
        return 1;
    };
    if !output.status.success() {
        print_error("Origin remote", "not configured (run `photon remote set-origin <url>`)");
        return 1;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if url.is_empty() {
        print_error("Origin remote", "not configured");
        return 1;
    }

    if let Ok(canonical) = fs::read_to_string(repository.join("Meta/Photon/origin.toml")) {
        let expected = canonical.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            if key.trim() == "repository" {
                Some(value.trim().trim_matches('"').to_owned())
            } else {
                None
            }
        });
        if let Some(expected) = expected {
            let normalize = |value: &str| {
                let https = value
                    .strip_prefix("git@github.com:")
                    .map(|rest| format!("https://github.com/{rest}"))
                    .unwrap_or_else(|| value.to_owned());
                https
                    .trim_end_matches(".git")
                    .trim_matches('/')
                    .to_ascii_lowercase()
            };
            if normalize(&url) != normalize(&expected) {
                print_error("Origin remote", &format!("{url} (expected {expected})"));
                return 1;
            }
        }
    }

    print_ok("Origin remote", &url);
    0
}

fn qt_requirement(repository: &Path) -> Option<Version> {
    let instructions = fs::read_to_string(repository.join("Documentation/BuildInstructionsLadybird.md")).ok()?;
    let marker = "Qt6.";
    let start = instructions.find(marker)? + 2;
    version_at_start(&instructions[start..])
}

fn report_build_directory(repository: &Path, preset: &str) {
    let relative = format!("Build/{preset}");
    if repository.join(&relative).is_dir() {
        print_ok("Build directory", &relative);
    } else {
        println!(
            "{} {:<15} {} (not created)",
            style("•").dim(),
            "Build directory",
            relative
        );
    }
}

fn first_version(text: &str) -> Option<Version> {
    for (index, character) in text.char_indices() {
        if character.is_ascii_digit() {
            if let Some(version) = version_at_start(&text[index..]) {
                return Some(version);
            }
        }
    }
    None
}

fn version_at_start(text: &str) -> Option<Version> {
    let mut components = text
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .next()?
        .split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components.next().and_then(|value| value.parse().ok()).unwrap_or(0);
    Some(Version::new(major, minor, patch))
}

fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
}

fn print_ok(label: &str, detail: &str) {
    println!("{} {:<15} {detail}", style("✓").green().bold(), label);
}

fn print_error(label: &str, detail: &str) {
    println!("{} {:<15} {detail}", style("✗").red().bold(), label);
}

fn print_version_error(label: &str, required: Version, found: Version) {
    println!("{} {label}", style("✗").red().bold());
    println!("  Required: {label} >= {required}");
    println!("  Found: {found}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_semantic_version_from_tool_output() {
        assert_eq!(first_version("cmake version 4.1.2\n"), Some(Version::new(4, 1, 2)));
    }

    #[test]
    fn accepts_versions_without_a_patch_component() {
        assert_eq!(first_version("Qt 6.11"), Some(Version::new(6, 11, 0)));
    }

    #[test]
    fn version_comparison_rejects_old_qt() {
        assert!(Version::new(6, 8, 3) < Version::new(6, 9, 0));
    }
}
