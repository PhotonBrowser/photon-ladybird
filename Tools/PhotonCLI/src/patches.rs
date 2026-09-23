// SPDX-License-Identifier: GPL-3.0-only
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use console::style;

use crate::metadata::{self, Patch};
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckoutState {
    Pristine,
    Materialized,
}

pub fn status(repository: &Path) -> Result<i32> {
    let patches = series(repository)?;
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    if patches.is_empty() {
        ui::header("Photon", Some("Patches"));
        ui::note("Series", "empty");
        return Ok(0);
    }

    ui::header("Photon", Some(&format!("Patches · {}", patches.len())));
    let owned_paths = patch_paths(repository, &patches)?;
    let state = checkout_state(repository, &patches).ok();
    if state.is_none() {
        ui::failure("Ladybird checkout is neither pristine nor the exact registered patch series");
    }
    for patch in &patches {
        let applied = match state {
            Some(CheckoutState::Materialized) => true,
            Some(CheckoutState::Pristine) => false,
            None => patch_is_applied(repository, patch)?,
        };
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
    let direct = changed_paths(repository)?
        .into_iter()
        .filter(|path| is_ladybird_path(path) && !owned_paths.contains(path))
        .collect::<Vec<_>>();
    let committed = committed_ladybird_paths(repository, &upstream.revision)?;
    if direct.is_empty() && committed.is_empty() {
        ui::ok("Direct Ladybird edits", "none outside registered patch paths");
    } else {
        ui::failure("Unrepresented direct Ladybird edits");
        for path in direct.into_iter().chain(committed) {
            println!("  {}", path.display());
        }
        return Ok(1);
    }
    if state.is_none() {
        return Ok(1);
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
    checkout_state(repository, &patches)
        .context("Ladybird checkout is neither pristine nor the exact registered patch series")?;
    let direct = changed_paths(repository)?
        .into_iter()
        .filter(|path| is_ladybird_path(path) && !owned_paths.contains(path))
        .collect::<Vec<_>>();
    let committed = committed_ladybird_paths(repository, &upstream.revision)?;
    if !direct.is_empty() || !committed.is_empty() {
        bail!(
            "unrepresented modifications to upstream Ladybird files since the recorded base:\n{}",
            direct
                .iter()
                .chain(committed.iter())
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

/// Return the Ladybird checkout to its recorded upstream state after validating
/// that every changed patch-owned file exactly matches the registered series.
pub fn unapply(repository: &Path) -> Result<i32> {
    let patches = series(repository)?;
    match checkout_state(repository, &patches)
        .context("Ladybird checkout is neither pristine nor the exact registered patch series")?
    {
        CheckoutState::Pristine => {
            ui::header("Photon", Some("Patches unapply"));
            ui::note("Ladybird", "already pristine at the recorded upstream base");
            return Ok(0);
        }
        CheckoutState::Materialized => {}
    }

    let owned_paths = patch_paths(repository, &patches)?;
    let staged = staged_paths(repository)?;
    let unexpected = staged
        .iter()
        .filter(|path| !owned_paths.contains(*path))
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        bail!(
            "staged changes outside the registered patch materialization prevent unapplying patches:\n{}",
            unexpected
                .iter()
                .map(|path| format!("  {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    for path in &staged {
        let status = Command::new("git")
            .args(["diff", "--quiet", "--"])
            .arg(path)
            .current_dir(repository)
            .status()?;
        match status.code() {
            Some(0) => {}
            Some(1) => bail!(
                "{} has staged content beyond the current patch materialization; unstage or commit it separately",
                path.display()
            ),
            _ => bail!("failed to compare staged and working-tree content for {}", path.display()),
        }
    }

    if !staged.is_empty() {
        for path in &staged {
            let object = format!("HEAD:{}", path.display());
            let tracked = Command::new("git")
                .args(["cat-file", "-e", &object])
                .current_dir(repository)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?
                .success();
            let mut unstage = Command::new("git");
            if tracked {
                unstage.args(["restore", "--staged", "--"]);
            } else {
                unstage.args(["rm", "--cached", "--force", "--"]);
            }
            unstage.arg(path).current_dir(repository);
            successful(&mut unstage, &format!("unstage materialized file {}", path.display()))?;
        }
    }

    ui::header("Photon", Some("Patches unapply"));
    for patch in patches.iter().rev() {
        successful(
            Command::new("git")
                .args(["apply", "--reverse"])
                .arg(patch_path(repository, patch)?)
                .current_dir(repository),
            &format!("unapply patch {}", patch.id),
        )?;
    }

    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    verify_pristine_files(repository, &patches, &upstream.revision)?;
    ui::ok("Ladybird", "patches unapplied; checkout is pristine at the recorded base");
    ui::hint("The next `./photon build` or `./photon run` will materialize the registered series again.");
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

fn verify_pristine_files(repository: &Path, patches: &[Patch], revision: &str) -> Result<()> {
    for path in patch_paths(repository, patches)? {
        let object = format!("{revision}:{}", path.display());
        let exists = Command::new("git")
            .args(["cat-file", "-e", &object])
            .current_dir(repository)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?
            .success();
        let expected = if exists {
            let output = Command::new("git")
                .args(["show", &object])
                .current_dir(repository)
                .output()?;
            if !output.status.success() {
                bail!("failed to read recorded upstream file {}", path.display());
            }
            Some(output.stdout)
        } else {
            None
        };
        let actual = fs::read(repository.join(&path)).ok();
        if actual != expected {
            bail!("{} differs from recorded pristine upstream", path.display());
        }
    }
    Ok(())
}

fn checkout_state(repository: &Path, patches: &[Patch]) -> Result<CheckoutState> {
    if verify_materialized_files(repository, patches).is_ok() {
        return Ok(CheckoutState::Materialized);
    }
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    verify_pristine_files(repository, patches, &upstream.revision)?;
    Ok(CheckoutState::Pristine)
}

/// Ensure the checked-out Ladybird files contain exactly the registered patch series.
/// A pristine base is patched in order; partial or divergent states are never overwritten.
pub fn ensure_materialized(repository: &Path) -> Result<()> {
    let patches = series(repository)?;
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    validate_recorded_base(repository, &upstream.revision)
        .context("run `./photon sync` to advance the base before building")?;
    match checkout_state(repository, &patches)? {
        CheckoutState::Materialized => {
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
        CheckoutState::Pristine => {}
    }
    let unexpected = changed_paths(repository)?
        .into_iter()
        .filter(|path| is_ladybird_path(path))
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        bail!(
            "Ladybird files have unrepresented working-tree changes; resolve them before materializing patches:\n{}",
            unexpected
                .iter()
                .map(|path| format!("  {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
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
        successful(
            Command::new("git")
                .arg("apply")
                .arg(patch_path(repository, patch)?)
                .current_dir(repository),
            &format!("materialize patch {}", patch.id),
        )?;
    }
    verify_materialized_files(repository, &patches)
}

pub(crate) fn validate_recorded_base(repository: &Path, revision: &str) -> Result<()> {
    let changed = committed_ladybird_paths(repository, revision)?;
    if !changed.is_empty() {
        bail!(
            "Ladybird files differ from the recorded upstream base {revision}:\n{}",
            changed
                .iter()
                .map(|path| format!("  {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    Ok(())
}

/// Allow sync to proceed when the only working-tree changes are the exact
/// registered Ladybird patch materialization. Photon source changes and staged
/// changes still require the contributor to commit or stash them first.
pub(crate) fn validate_sync_worktree(repository: &Path) -> Result<bool> {
    let staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repository)
        .status()?;
    if !staged.success() {
        bail!("staged changes are present; commit or stash them before syncing");
    }

    let patches = series(repository)?;
    if checkout_state(repository, &patches)? == CheckoutState::Pristine {
        if changed_paths(repository)?.is_empty() {
            return Ok(false);
        }
        bail!("working tree is not clean; commit or stash your work before syncing");
    }

    let owned_paths = patch_paths(repository, &patches)?;
    let unexpected = changed_paths(repository)?
        .into_iter()
        .filter(|path| !is_ladybird_path(path) || !owned_paths.contains(path))
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        bail!(
            "working tree has changes outside the registered patch materialization:\n{}",
            unexpected
                .iter()
                .map(|path| format!("  {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    Ok(true)
}

/// Recognize the clean, unpatched upstream checkout left behind when a sync
/// stopped so registered patches can be refreshed against the merged target.
pub(crate) fn is_pristine_at_revision(repository: &Path, revision: &str) -> Result<bool> {
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(repository)
        .output()?;
    if !status.status.success() {
        bail!("failed to inspect working tree");
    }
    if !status.stdout.is_empty() {
        return Ok(false);
    }

    let paths = patch_paths(repository, &series(repository)?)?;
    if paths.is_empty() {
        return Ok(true);
    }

    let mut diff = Command::new("git");
    diff.args(["diff", "--quiet", revision, "--"]).current_dir(repository);
    for path in paths {
        diff.arg(path);
    }
    let result = diff.status()?;
    match result.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("failed to compare Ladybird checkout with upstream {revision}"),
    }
}

/// Recognize the safe intermediate state after sync merged upstream but the
/// contributor needed to refresh the registered series. Only unstaged patch
/// metadata/files may differ; all Ladybird patch-owned paths must still be
/// pristine at the merged revision.
pub(crate) fn is_patch_refresh_worktree(repository: &Path, revision: &str) -> Result<bool> {
    let staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repository)
        .status()?;
    if !staged.success() {
        return Ok(false);
    }

    let changed = changed_paths(repository)?;
    if changed.is_empty()
        || changed
            .iter()
            .any(|path| path != Path::new("Patches/series.toml") && !path.starts_with("Patches/ladybird"))
    {
        return Ok(false);
    }

    verify_pristine_files(repository, &series(repository)?, revision)?;
    is_pristine_ladybird_at_revision(repository, revision)
}

fn is_pristine_ladybird_at_revision(repository: &Path, revision: &str) -> Result<bool> {
    let owned_paths = patch_paths(repository, &series(repository)?)?;
    let changed = changed_paths(repository)?;
    if changed
        .iter()
        .any(|path| is_ladybird_path(path) && !owned_paths.contains(path))
    {
        return Ok(false);
    }
    let committed = committed_ladybird_paths(repository, revision)?;
    Ok(committed.is_empty())
}

/// Remove an already verified patch materialization so Git can merge pristine
/// upstream files. Patches are reversed in the opposite order from the series.
pub(crate) fn unmaterialize_for_sync(repository: &Path) -> Result<()> {
    if !validate_sync_worktree(repository)? {
        return Ok(());
    }
    for patch in series(repository)?.iter().rev() {
        successful(
            Command::new("git")
                .args(["apply", "--reverse"])
                .arg(patch_path(repository, patch)?)
                .current_dir(repository),
            &format!("remove patch {} before upstream merge", patch.id),
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

fn staged_paths(repository: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--no-renames"])
        .current_dir(repository)
        .output()?;
    if !output.status.success() {
        bail!("failed to inspect staged changes");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(PathBuf::from)
        .collect())
}

fn committed_ladybird_paths(repository: &Path, revision: &str) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "--no-renames", revision, "HEAD"])
        .current_dir(repository)
        .output()?;
    if !output.status.success() {
        bail!("failed to compare HEAD with recorded Ladybird base {revision}");
    }
    let owned_paths = patch_paths(repository, &series(repository)?)?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(PathBuf::from)
        .filter(|path| is_ladybird_path(path) && !owned_paths.contains(path))
        .collect())
}

fn is_ladybird_path(path: &Path) -> bool {
    !path.starts_with("Photon")
        && !path.starts_with("Tools/PhotonCLI")
        && !path.starts_with("Documentation/Photon")
        && !path.starts_with("Meta/Photon")
        && !path.starts_with("Patches")
        && path != Path::new(".gitignore")
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
