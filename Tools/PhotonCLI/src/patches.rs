use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use console::style;

use crate::metadata::{self, Patch};
use crate::ui;

pub fn status(repository: &Path) -> Result<i32> {
    let patches = series(repository)?;
    if patches.is_empty() {
        ui::header("Photon", Some("Patches"));
        ui::note("Series", "empty");
        return Ok(0);
    }

    ui::header("Photon", Some(&format!("Patches · {}", patches.len())));
    for patch in patches {
        let path = patch_path(repository, &patch)?;
        let applied = Command::new("git")
            .args(["apply", "--reverse", "--check"])
            .arg(&path)
            .current_dir(repository)
            .status()?
            .success();
        if applied {
            println!(
                "  {} {:<8} {} {}",
                style("✓").green().bold(),
                patch.id,
                patch.description,
                style(format!("({})", patch.area)).dim()
            );
        } else {
            println!(
                "  {} {:<8} {} {}",
                style("○").yellow().bold(),
                patch.id,
                patch.description,
                style(format!("({}) · not applied or diverged", patch.area)).dim()
            );
        }
    }
    Ok(0)
}

pub fn check(repository: &Path) -> Result<i32> {
    let patches = series(repository)?;
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    let temporary = std::env::temp_dir().join(format!("photon-patches-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)?;
    }
    fs::create_dir(&temporary)?;

    let result = check_in(repository, &temporary, &upstream.revision, &patches);
    fs::remove_dir_all(&temporary).context("failed to remove temporary patch checkout")?;
    result?;

    let suffix = if patches.len() == 1 { "patch" } else { "patches" };
    ui::header("Photon", Some("Patches check"));
    ui::ok(
        "Applies cleanly",
        format!("{} ({} {suffix})", ui::sha(&upstream.revision), patches.len()),
    );
    ui::hint(format!("Base: {}", upstream.revision));
    Ok(0)
}

pub(crate) fn check_in(repository: &Path, temporary: &Path, revision: &str, patches: &[Patch]) -> Result<()> {
    let archive = temporary.join("base.tar");
    successful(
        Command::new("git")
            .args(["archive", "--format=tar", "-o"])
            .arg(&archive)
            .arg(revision)
            .current_dir(repository),
        "export recorded Ladybird base",
    )?;
    successful(
        Command::new("tar").args(["-xf"]).arg(&archive).arg("-C").arg(temporary),
        "extract recorded Ladybird base",
    )?;
    fs::remove_file(archive)?;

    for patch in patches {
        let path = patch_path(repository, patch)?;
        successful(
            Command::new("git")
                .args(["apply", "--check"])
                .arg(&path)
                .current_dir(temporary),
            &format!("check patch {}", patch.id),
        )?;
        successful(
            Command::new("git").args(["apply"]).arg(path).current_dir(temporary),
            &format!("apply patch {}", patch.id),
        )?;
    }
    Ok(())
}

pub(crate) fn series(repository: &Path) -> Result<Vec<Patch>> {
    metadata::read_patch_series(&repository.join("Patches/series.toml"))
}

fn patch_path(repository: &Path, patch: &Patch) -> Result<PathBuf> {
    let path = repository.join("Patches").join(&patch.file);
    if !path.is_file() {
        bail!("missing patch file: {}", path.display());
    }
    Ok(path)
}

fn successful(command: &mut Command, operation: &str) -> Result<()> {
    let status = command.status().with_context(|| format!("failed to {operation}"))?;
    if !status.success() {
        bail!("failed to {operation}");
    }
    Ok(())
}
