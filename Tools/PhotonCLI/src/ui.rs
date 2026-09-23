// SPDX-License-Identifier: GPL-3.0-only
use console::style;
use std::fmt::Display;

/// Fixed width for `label` columns so key/value rows line up.
pub const LABEL_WIDTH: usize = 18;

/// Print a top-level header, e.g. `Photon` with optional dimmed context.
pub fn header(title: &str, context: Option<&str>) {
    match context {
        Some(context) => println!("{} {}\n", style(title).bold(), style(format!("· {context}")).dim()),
        None => println!("{}\n", style(title).bold()),
    }
}

/// Print a dimmed section divider, e.g. `Toolchain`.
pub fn section(title: &str) {
    println!("\n{}", style(title).bold());
}

/// Print an aligned `label  value` row. Labels are dimmed; values keep caller styling.
pub fn kv(label: &str, value: impl Display) {
    println!("  {:<width$} {value}", style(label).dim(), width = LABEL_WIDTH);
}

/// Success row: `✓ label  detail`.
pub fn ok(label: &str, detail: impl Display) {
    println!(
        "  {} {:<width$} {detail}",
        style("✓").green().bold(),
        label,
        width = LABEL_WIDTH
    );
}

/// Failure row: `✗ label  detail`.
pub fn fail(label: &str, detail: impl Display) {
    println!(
        "  {} {:<width$} {detail}",
        style("✗").red().bold(),
        label,
        width = LABEL_WIDTH
    );
}

/// Neutral row: `• label  detail` with a dimmed detail.
pub fn note(label: &str, detail: impl Display) {
    println!(
        "  {} {:<width$} {}",
        style("•").dim(),
        label,
        style(format!("{detail}")).dim(),
        width = LABEL_WIDTH
    );
}

/// Action step: `→ message` in cyan.
pub fn step(message: impl Display) {
    println!("  {} {message}", style("→").cyan().bold());
}

/// Hint line: dimmed `hint: message` continuation.
pub fn hint(message: impl Display) {
    println!("    {}", style(format!("{message}")).dim());
}

/// Final success line: `✓ message`.
pub fn success(message: impl Display) {
    println!("\n{} {message}", style("✓").green().bold());
}

/// Final failure line: `✗ message`.
pub fn failure(message: impl Display) {
    eprintln!("\n{} {message}", style("✗").red().bold());
}

/// Shorten a commit SHA for display without losing uniqueness in context.
pub fn short_sha(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

/// Style a SHA in yellow for scanability.
pub fn sha(sha: &str) -> String {
    style(short_sha(sha)).yellow().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_full_shas_to_twelve_characters() {
        assert_eq!(short_sha("6bb3324f6d9b48c7ec2f7e5b7c6b7452d8401fa7"), "6bb3324f6d9b");
    }

    #[test]
    fn leaves_short_values_untouched() {
        assert_eq!(short_sha("abc"), "abc");
    }
}
