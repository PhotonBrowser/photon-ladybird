use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use crate::process::ProcessOutcome;
use crate::ui;

const FAILURE_TAIL_LINES: usize = 16;

#[derive(Debug, PartialEq, Eq)]
struct NinjaProgress<'a> {
    completed: u64,
    total: u64,
    description: &'a str,
}

pub struct BuildDisplay {
    label: &'static str,
    verbose: bool,
    log_path: PathBuf,
    progress: ProgressBar,
    has_exact_progress: bool,
}

impl BuildDisplay {
    pub fn new(label: &'static str, preset: &str, verbose: bool, log_path: &Path) -> Self {
        ui::header("Photon", Some(&format!("{label} · {preset} · Qt")));
        ui::kv("Log", log_path.display());
        println!();

        let progress = ProgressBar::new_spinner();
        progress.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}\n  {elapsed_precise} elapsed")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        progress.set_message("Preparing dependencies and configuring");
        if !verbose {
            progress.enable_steady_tick(Duration::from_millis(100));
        }

        Self {
            label,
            verbose,
            log_path: log_path.to_path_buf(),
            progress,
            has_exact_progress: false,
        }
    }

    pub fn observe(&mut self, line: &str) {
        if self.verbose {
            return;
        }

        if let Some(update) = parse_ninja_progress(line) {
            if !self.has_exact_progress {
                self.progress.set_style(
                    ProgressStyle::with_template(
                        "{spinner:.cyan} {msg}\n  {pos}/{len} targets · {percent}% · {elapsed_precise}\n  {wide_bar:.cyan/blue}",
                    )
                    .unwrap_or_else(|_| ProgressStyle::default_bar()),
                );
                self.has_exact_progress = true;
            }
            self.progress.set_length(update.total);
            self.progress.set_position(update.completed);
            let detail = concise_build_detail(update.description);
            self.progress.set_message(format!("{} · {detail}", self.label));
        } else if line.contains("Test project") || line.trim_start().starts_with("Start ") {
            self.progress.set_message("Running tests");
        }
    }

    pub fn finish(&self, outcome: &ProcessOutcome, elapsed: Duration, target: Option<&str>) {
        self.progress.finish_and_clear();
        if outcome.code == 0 {
            println!(
                "{} {} · {}",
                style("✓").green().bold(),
                self.label.trim_end_matches("ing Photon"),
                style(format_duration(elapsed)).dim()
            );
            ui::hint(format!("Log: {}", self.log_path.display()));
            return;
        }

        eprintln!(
            "{} {} · exited with code {}",
            style("✗").red().bold(),
            self.label,
            outcome.code
        );
        if let Some(target) = target {
            eprintln!("  {} {target}", style("Target").dim());
        }
        if !outcome.last_lines.is_empty() {
            eprintln!("\n  {}", style("Last output").bold());
            for line in useful_tail(&outcome.last_lines) {
                eprintln!("    {line}");
            }
        }
        eprintln!("\n  {} {}", style("Full log").dim(), self.log_path.display());
    }
}

fn parse_ninja_progress(line: &str) -> Option<NinjaProgress<'_>> {
    let marker = "[PHOTON ";
    let start = line.find(marker)? + marker.len();
    let remainder = &line[start..];
    let end = remainder.find(']')?;
    let (completed, total) = remainder[..end].split_once('/')?;

    Some(NinjaProgress {
        completed: completed.parse().ok()?,
        total: total.parse().ok()?,
        description: remainder[end + 1..].trim(),
    })
}

fn concise_build_detail(description: &str) -> &str {
    description
        .split_whitespace()
        .find(|word| {
            let word = word.trim_end_matches([')', ':']);
            [".c", ".cc", ".cpp", ".cxx", ".rs"]
                .iter()
                .any(|extension| word.ends_with(extension))
        })
        .and_then(|word| word.rsplit('/').next())
        .unwrap_or(description)
}

fn useful_tail(lines: &VecDeque<String>) -> impl Iterator<Item = &str> {
    lines
        .iter()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(FAILURE_TAIL_LINES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(String::as_str)
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_photon_ninja_progress() {
        let progress =
            parse_ninja_progress("[PHOTON 1428/1693] CXX WebContent.cpp").expect("marked progress should parse");

        assert_eq!(
            progress,
            NinjaProgress {
                completed: 1428,
                total: 1693,
                description: "CXX WebContent.cpp",
            }
        );
    }

    #[test]
    fn ignores_unmarked_fraction_like_output() {
        assert_eq!(parse_ninja_progress("[12/20] unrelated"), None);
    }

    #[test]
    fn extracts_source_file_from_ninja_description() {
        assert_eq!(
            concise_build_detail("Building CXX object Libraries/Foo/WebContent.cpp"),
            "WebContent.cpp"
        );
    }

    #[test]
    fn formats_elapsed_time_without_inventing_precision() {
        assert_eq!(format_duration(Duration::from_secs(222)), "03:42");
    }
}
