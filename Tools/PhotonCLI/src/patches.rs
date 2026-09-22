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
    let owned_paths = patch_paths(repository, &patches)?;
    let mut all_applied = true;
    for patch in &patches {
        let path = patch_path(repository, patch)?;
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
            all_applied = false;
            println!(
                "  {} {:<8} {} {}",
                style("○").yellow().bold(),
                patch.id,
                patch.description,
                style(format!("({}) · not applied or diverged", patch.area)).dim()
            );
        }
    }
    let direct = changed_paths(repository)?
        .into_iter()
        .filter(|path| is_ladybird_path(path) && !owned_paths.contains(path))
        .collect::<Vec<_>>();
    if direct.is_empty() {
        ui::ok("Direct Ladybird edits", "none outside registered patch paths");
    } else {
        ui::failure("Unrepresented direct Ladybird edits");
        for path in direct {
            println!("  {}", path.display());
        }
        return Ok(1);
    }
    if all_applied {
        if let Err(error) = verify_materialized_files(repository, &patches) {
            ui::failure(format!(
                "Materialized files differ from the registered series: {error:#}"
            ));
            return Ok(1);
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
    let owned_paths = patch_paths(repository, &patches)?;
    if patches
        .iter()
        .map(|patch| patch_is_applied(repository, patch))
        .collect::<Result<Vec<_>>>()?
        .iter()
        .all(|applied| *applied)
    {
        verify_materialized_files(repository, &patches)?;
    }
    let direct = changed_paths(repository)?
        .into_iter()
        .filter(|path| is_ladybird_path(path) && !owned_paths.contains(path))
        .collect::<Vec<_>>();
    if !direct.is_empty() {
        bail!(
            "unrepresented direct modifications to upstream Ladybird files:\n{}",
            direct
                .iter()
                .map(|path| format!("  {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    let suffix = if patches.len() == 1 { "patch" } else { "patches" };
    ui::header("Photon", Some("Patches check"));
    ui::ok(
        "Applies cleanly",
        format!("{} ({} {suffix})", ui::sha(&upstream.revision), patches.len()),
    );
    ui::hint(format!("Base: {}", upstream.revision));
    Ok(0)
}

fn verify_materialized_files(repository: &Path, patches: &[Patch]) -> Result<()> {
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    let temporary = std::env::temp_dir().join(format!("photon-materialized-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)?;
    }
    fs::create_dir(&temporary)?;
    let result = (|| {
        check_in(repository, &temporary, &upstream.revision, patches)?;
        for path in patch_paths(repository, patches)? {
            let expected = fs::read(temporary.join(&path))
                .with_context(|| format!("missing materialized file {}", path.display()))?;
            let actual = fs::read(repository.join(&path))
                .with_context(|| format!("missing checkout file {}", path.display()))?;
            if actual != expected {
                bail!("{} contains changes beyond the registered patch series", path.display());
            }
        }
        Ok(())
    })();
    fs::remove_dir_all(&temporary).context("failed to remove temporary materialization")?;
    result
}

/// Ensure the checked-out Ladybird files contain exactly the registered patch series.
/// A pristine base is patched in order; partial or divergent states are never overwritten.
pub fn ensure_materialized(repository: &Path) -> Result<()> {
    let patches = series(repository)?;
    let applied = patches
        .iter()
        .map(|patch| patch_is_applied(repository, patch))
        .collect::<Result<Vec<_>>>()?;
    if applied.iter().all(|state| *state) {
        verify_materialized_files(repository, &patches)?;
        let owned_paths = patch_paths(repository, &patches)?;
        let direct = changed_paths(repository)?
            .into_iter()
            .filter(|path| is_ladybird_path(path) && !owned_paths.contains(path))
            .collect::<Vec<_>>();
        if !direct.is_empty() {
            bail!(
                "unrepresented direct modifications to upstream Ladybird files: {}",
                direct
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        return Ok(());
    }
    if applied.iter().any(|state| *state) {
        bail!("patch series is only partially materialized; run `./photon patches status` and resolve it manually");
    }
    for patch in &patches {
        let path = patch_path(repository, patch)?;
        successful(
            Command::new("git")
                .args(["apply", "--check"])
                .arg(&path)
                .current_dir(repository),
            &format!("check patch {} against checkout", patch.id),
        )?;
    }
    for patch in &patches {
        successful(
            Command::new("git")
                .arg("apply")
                .arg(patch_path(repository, patch)?)
                .current_dir(repository),
            &format!("materialize patch {}", patch.id),
        )?;
    }
    Ok(())
}

fn patch_is_applied(repository: &Path, patch: &Patch) -> Result<bool> {
    let path = patch_path(repository, patch)?;
    Ok(Command::new("git")
        .args(["apply", "--reverse", "--check"])
        .arg(path)
        .current_dir(repository)
        .status()?
        .success())
}

fn patch_paths(repository: &Path, patches: &[Patch]) -> Result<std::collections::HashSet<PathBuf>> {
    let mut paths = std::collections::HashSet::new();
    for patch in patches {
        let output = Command::new("git")
            .args(["apply", "--numstat"])
            .arg(patch_path(repository, patch)?)
            .current_dir(repository)
            .output()?;
        if !output.status.success() {
            bail!("failed to inspect patch {}", patch.id);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if let Some(name) = line.split('\t').nth(2) {
                paths.insert(PathBuf::from(name));
            }
        }
    }
    Ok(paths)
}

fn changed_paths(repository: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(repository)
        .output()?;
    if !output.status.success() {
        bail!("failed to inspect working tree");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.get(3..).map(str::trim).map(PathBuf::from))
        .collect())
}

fn is_ladybird_path(path: &Path) -> bool {
    !path.starts_with("Photon")
        && !path.starts_with("Tools/PhotonCLI")
        && !path.starts_with("Documentation/Photon")
        && !path.starts_with("Meta/Photon")
        && !path.starts_with("Patches")
        && path != Path::new("photon")
        && path != Path::new("agents.md")
        && path != Path::new("AGENTS.md")
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
