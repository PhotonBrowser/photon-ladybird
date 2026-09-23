// SPDX-License-Identifier: GPL-3.0-only
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use console::style;

use crate::metadata::{self, Patch};
use crate::ui;

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
    verify_pristine_files(repository, &patches, &upstream.revision)
        .context("canonical Ladybird source must remain pristine; use `./photon engine edit`")?;
    let materialized = crate::engine::status_tree(repository)?;
    for patch in &patches {
        if let Some(tree) = materialized {
            println!(
                "  {} {:<8} {} {}",
                style("✓").green().bold(),
                patch.id,
                patch.description,
                style(format!("({} · {tree})", patch.area)).dim()
            );
        } else {
            println!(
                "  {} {:<8} {} {}",
                style("○").yellow().bold(),
                patch.id,
                patch.description,
                style(format!("({} · registered)", patch.area)).dim()
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
    let edit_paths = unrepresented_edit_paths(repository)?;
    if edit_paths.is_empty() {
        ui::ok("Engine edit tree", "no uncaptured changes");
    } else {
        ui::failure("Uncaptured engine edits in .photon/worktree");
        for path in edit_paths {
            println!("  {}", path.display());
        }
        return Ok(1);
    }
    Ok(0)
}

fn unrepresented_edit_paths(repository: &Path) -> Result<Vec<PathBuf>> {
    let edit_tree = crate::engine::edit_tree(repository);
    if !edit_tree.exists() {
        return Ok(Vec::new());
    }
    crate::engine::verify_edit_tree(repository)?;
    let patches = series(repository)?;
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    let temporary = std::env::temp_dir().join(format!("photon-edit-status-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)?;
    }
    fs::create_dir(&temporary)?;
    let baseline = temporary.join("baseline");
    fs::create_dir(&baseline)?;
    check_in(repository, &baseline, &upstream.revision, &patches)?;

    let mut changed = Vec::new();
    for path in changed_paths(&edit_tree)? {
        if !is_ladybird_path(&path) {
            continue;
        }
        if fs::read(baseline.join(&path)).ok() != fs::read(edit_tree.join(&path)).ok() {
            changed.push(path);
        }
    }
    fs::remove_dir_all(&temporary)?;
    Ok(changed)
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
    verify_pristine_files(repository, &patches, &upstream.revision)
        .context("canonical Ladybird source must remain pristine; use `./photon engine edit`")?;
    crate::engine::materialized(repository)?;
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
    let edit_paths = unrepresented_edit_paths(repository)?;
    if !edit_paths.is_empty() {
        bail!(
            "uncaptured engine edits remain in .photon/worktree:\n{}",
            edit_paths
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

/// Capture the delta in the engine edit worktree as the next registered patch.
pub fn capture(repository: &Path, name: &str, area: &str) -> Result<i32> {
    let name = name.trim();
    let area = area.trim();
    if name.is_empty() || area.is_empty() {
        bail!("patch name and area must not be empty");
    }
    if name.chars().any(char::is_control) || area.chars().any(char::is_control) {
        bail!("patch name and area must not contain control characters");
    }

    let edit_tree = crate::engine::edit_tree(repository);
    if !edit_tree.exists() {
        bail!("no engine edit tree exists; run `./photon engine edit` first");
    }
    let patches = series(repository)?;
    if patches.is_empty() {
        bail!("cannot capture a patch without an existing registered series");
    }
    crate::engine::verify_edit_tree(repository)?;

    let staged_engine_paths = staged_paths(&edit_tree)?;
    if !staged_engine_paths.is_empty() {
        bail!(
            "staged changes in the engine edit tree prevent capture; unstage them first:\n{}",
            staged_engine_paths
                .iter()
                .map(|path| format!("  {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    validate_recorded_base(repository, &upstream.revision)?;
    let temporary = std::env::temp_dir().join(format!("photon-capture-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)?;
    }
    fs::create_dir(&temporary)?;
    let baseline = temporary.join("baseline");
    fs::create_dir(&baseline)?;
    check_in(repository, &baseline, &upstream.revision, &patches)?;

    let mut patch_contents = String::new();
    for path in changed_paths(&edit_tree)? {
        if !is_ladybird_path(&path) {
            continue;
        }
        let baseline_file = baseline.join(&path);
        let checkout_file = edit_tree.join(&path);
        let baseline_exists = baseline_file.is_file();
        let checkout_exists = checkout_file.is_file();
        if !baseline_exists && !checkout_exists {
            continue;
        }

        let baseline_arg = if baseline_exists {
            baseline_file.clone()
        } else {
            PathBuf::from("/dev/null")
        };
        let checkout_arg = if checkout_exists {
            checkout_file
        } else {
            PathBuf::from("/dev/null")
        };
        let output = Command::new("diff")
            .arg("-u")
            .arg("--label")
            .arg(if baseline_exists {
                format!("a/{}", path.display())
            } else {
                "/dev/null".to_owned()
            })
            .arg("--label")
            .arg(if checkout_exists {
                format!("b/{}", path.display())
            } else {
                "/dev/null".to_owned()
            })
            .arg(baseline_arg)
            .arg(checkout_arg)
            .output()
            .context("failed to create a source diff; install the diff utility")?;
        if !output.status.success() && output.status.code() != Some(1) {
            bail!("failed to compare Ladybird source file {}", path.display());
        }
        patch_contents.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if patch_contents.is_empty() {
        fs::remove_dir_all(&temporary)?;
        ui::header("Photon", Some("Patches already registered"));
        ui::note("Capture", "no unrepresented Ladybird edits; no duplicate patch created");
        return Ok(0);
    }

    let next_id = patches
        .iter()
        .filter_map(|patch| patch.id.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let id = format!("{next_id:04}");
    let slug = patch_slug(name);
    if slug.is_empty() {
        fs::remove_dir_all(&temporary)?;
        bail!("patch name must contain at least one letter or number");
    }
    let file = format!("ladybird/{id}-{slug}.patch");
    let patch_file = repository.join("Patches").join(&file);
    if patch_file.exists() {
        fs::remove_dir_all(&temporary)?;
        bail!("patch file already exists: {}", patch_file.display());
    }
    let candidate = temporary.join("candidate.patch");
    fs::write(&candidate, &patch_contents)?;
    successful(
        Command::new("git")
            .args(["apply", "--check"])
            .arg(&candidate)
            .current_dir(&baseline),
        "validate captured source changes against the registered patch series",
    )?;
    successful(
        Command::new("git").arg("apply").arg(&candidate).current_dir(&baseline),
        "apply captured changes to the temporary series checkout",
    )?;
    crate::engine::discard_build_tree(repository)?;

    fs::write(&patch_file, patch_contents)?;
    let series_file = repository.join("Patches/series.toml");
    let mut manifest = fs::read_to_string(&series_file)?;
    manifest.push_str(&format!(
        "\n[[patches]]\nid = \"{id}\"\nfile = \"{file}\"\ndescription = \"{}\"\narea = \"{}\"\n",
        toml_string(name),
        toml_string(area)
    ));
    fs::write(&series_file, manifest)?;

    let updated_patches = series(repository)?;
    crate::engine::verify_edit_tree_with_series(repository, &edit_tree, &updated_patches)?;
    fs::remove_dir_all(&temporary).context("failed to remove temporary patch capture files")?;

    ui::header("Photon", Some("Patch captured"));
    ui::ok(&id, format!("{} · {}", name, file));
    ui::ok(
        "Engine edit tree",
        "captured change remains available and is represented by the new patch",
    );
    Ok(0)
}

fn patch_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    slug
}

fn toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn verify_materialized_files(repository: &Path, patches: &[Patch]) -> Result<()> {
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

/// Ensure a generated build tree exists with exactly the registered patch series.
pub fn ensure_materialized(repository: &Path) -> Result<()> {
    crate::engine::ensure_build_tree(repository).map(|_| ())
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
pub(crate) fn validate_sync_worktree(repository: &Path) -> Result<()> {
    let staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repository)
        .status()?;
    if !staged.success() {
        bail!("staged changes are present; commit or stash them before syncing");
    }

    let patches = series(repository)?;
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    verify_pristine_files(repository, &patches, &upstream.revision)
        .context("canonical Ladybird source must remain pristine; use `./photon engine edit`")?;
    if !changed_paths(repository)?.is_empty() {
        bail!("working tree is not clean; commit or stash your work before syncing");
    }
    Ok(())
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

pub(crate) fn patch_paths(repository: &Path, patches: &[Patch]) -> Result<std::collections::HashSet<PathBuf>> {
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
