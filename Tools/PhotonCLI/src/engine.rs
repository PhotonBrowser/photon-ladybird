// SPDX-License-Identifier: GPL-3.0-only
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::metadata;
use crate::metadata::Patch;
use crate::patches;
use crate::ui;

const BUILD_TREE: &str = "Build/Source";
const EDIT_TREE: &str = ".photon/worktree";

pub fn materialize(repository: &Path) -> Result<i32> {
    ensure_tree(repository, &build_tree(repository), true)?;
    ui::header("Photon", Some("Engine materialized"));
    ui::ok("Source", BUILD_TREE);
    ui::hint("Generated source lives outside the canonical Ladybird worktree.");
    Ok(0)
}

pub fn edit(repository: &Path) -> Result<i32> {
    ensure_tree(repository, &edit_tree(repository), true)?;
    ui::header("Photon", Some("Engine edit worktree"));
    ui::ok("Source", EDIT_TREE);
    ui::hint("Edit Ladybird files here, then run `./photon patches capture \"description\"`.");
    Ok(0)
}

pub fn clean(repository: &Path) -> Result<i32> {
    for tree in [build_tree(repository), edit_tree(repository)] {
        let alias = tree_alias(repository, &tree)?;
        if !tree.exists() && alias.exists() && !fs::symlink_metadata(&alias)?.file_type().is_symlink() {
            ensure_tree(repository, &tree, true)?;
        }
    }
    for (tree, alias) in [
        (build_tree(repository), repository.join(BUILD_TREE)),
        (edit_tree(repository), repository.join(EDIT_TREE)),
    ] {
        if !tree.exists() {
            remove_alias_if_dangling(&alias)?;
            continue;
        }
        verify_tree(repository, &tree)?;
        park_build_directory(repository, &tree)?;
        let tree_argument = path_arg(&tree, repository)?;
        git_success(
            repository,
            &["worktree", "remove", "--force", &tree_argument],
            &format!("remove generated engine tree {}", tree.display()),
        )?;
        remove_alias_if_dangling(&alias)?;
    }
    ui::header("Photon", Some("Engine clean"));
    ui::ok(
        "Generated source",
        "removed; build artifacts parked beside the checkout",
    );
    Ok(0)
}

pub(crate) fn ensure_build_tree(repository: &Path) -> Result<PathBuf> {
    let editing = edit_tree(repository);
    if editing.exists() {
        verify_edit_tree(repository)?;
        ensure_build_directory(repository, &editing)?;
        return Ok(editing);
    }
    let tree = build_tree(repository);
    ensure_tree(repository, &tree, true)?;
    Ok(tree)
}

pub(crate) fn edit_tree(repository: &Path) -> PathBuf {
    worktree_root(repository).join("Edit")
}

pub(crate) fn has_build_tree(repository: &Path) -> bool {
    build_tree(repository).exists()
}

pub(crate) fn has_edit_tree(repository: &Path) -> bool {
    edit_tree(repository).exists()
}

pub(crate) fn materialized(repository: &Path) -> Result<bool> {
    let tree = build_tree(repository);
    if !tree.exists() {
        return Ok(false);
    }
    verify_tree(repository, &tree)?;
    Ok(true)
}

pub(crate) fn status_tree(repository: &Path) -> Result<Option<&'static str>> {
    let build = build_tree(repository);
    if build.exists() {
        verify_tree(repository, &build)?;
        return Ok(Some(BUILD_TREE));
    }
    let edit = edit_tree(repository);
    if edit.exists() {
        verify_edit_tree(repository)?;
        return Ok(Some(EDIT_TREE));
    }
    Ok(None)
}

pub(crate) fn verify_edit_tree(repository: &Path) -> Result<()> {
    let tree = edit_tree(repository);
    if !tree.exists() {
        bail!("engine edit tree is missing; run `./photon engine edit` first");
    }
    verify_edit_tree_path(repository, &tree)
}

fn verify_edit_tree_path(repository: &Path, tree: &Path) -> Result<()> {
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    let revision = git_output(tree, &["rev-parse", "HEAD"])?;
    if revision != upstream.revision {
        bail!(
            "engine edit tree is based on {}, expected recorded upstream {}; clean and recreate it before capture",
            revision,
            upstream.revision
        );
    }
    Ok(())
}

pub(crate) fn verify_edit_tree_with_series(repository: &Path, tree: &Path, patches: &[Patch]) -> Result<()> {
    verify_tree_with_series(repository, tree, patches)
}

pub(crate) fn verify_build_tree_with_series(repository: &Path, patches: &[Patch]) -> Result<()> {
    verify_tree_with_series(repository, &build_tree(repository), patches)
}

/// Keep a matching tree, safely reconcile a known omitted patch, or discard a
/// build tree that still matches the old series.
pub(crate) fn prepare_build_tree_for_capture(
    repository: &Path,
    old_patches: &[Patch],
    new_patches: &[Patch],
) -> Result<()> {
    let tree = build_tree(repository);
    if !tree.exists() {
        return Ok(());
    }

    if verify_tree_with_series(repository, &tree, new_patches).is_ok() {
        return Ok(());
    }

    if verify_tree_with_series(repository, &tree, old_patches).is_ok() {
        return discard_build_tree(repository);
    }

    if reconcile_tree_with_one_missing_patch(repository, &tree, new_patches)? {
        return Ok(());
    }

    bail!("generated build source matches neither the current nor proposed patch series")
}

/// Reconcile a tree only when every patch-owned file exactly matches the
/// proposed series with one registered patch omitted.
fn reconcile_tree_with_one_missing_patch(repository: &Path, tree: &Path, patches: &[Patch]) -> Result<bool> {
    let paths = patches::patch_paths(repository, patches)?;
    if paths.is_empty() {
        return Ok(false);
    }

    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    if git_output(tree, &["rev-parse", "HEAD"])? != upstream.revision {
        return Ok(false);
    }
    let full = materialize_for_comparison(repository, &upstream.revision, patches)?;
    let differing_paths = differing_materialized_paths(&full, tree, &paths);
    fs::remove_dir_all(&full)?;
    let differing_paths = differing_paths?;
    if differing_paths.is_empty() {
        return Ok(false);
    }

    for omitted_index in (0..patches.len()).rev() {
        let omitted_patch_paths = patches::patch_paths(repository, &[patches[omitted_index].clone()])?;
        if !differing_paths.iter().all(|path| omitted_patch_paths.contains(path)) {
            continue;
        }

        let mut subset = patches.to_vec();
        subset.remove(omitted_index);
        let expected = match materialize_for_comparison(repository, &upstream.revision, &subset) {
            Ok(expected) => expected,
            Err(_) => continue,
        };
        let matches = differing_materialized_paths(&expected, tree, &paths);
        fs::remove_dir_all(&expected)?;
        let matches = matches?.is_empty();
        if !matches {
            continue;
        }

        verify_tree_status_paths(tree, &paths)?;
        let mut affected_paths = patches::patch_paths(repository, &subset)?;
        affected_paths.extend(paths);
        reconcile_tree_patch_series(repository, tree, patches, &affected_paths)?;
        verify_tree_with_series(repository, tree, patches)?;
        return Ok(true);
    }

    Ok(false)
}

fn materialize_for_comparison(repository: &Path, revision: &str, patches: &[Patch]) -> Result<PathBuf> {
    let temporary = std::env::temp_dir().join(format!("photon-tree-check-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)?;
    }
    fs::create_dir(&temporary)?;
    if let Err(error) = patches::check_in(repository, &temporary, revision, patches) {
        fs::remove_dir_all(&temporary)?;
        return Err(error);
    }
    Ok(temporary)
}

fn differing_materialized_paths(expected: &Path, actual: &Path, paths: &HashSet<PathBuf>) -> Result<Vec<PathBuf>> {
    let mut differing_paths = Vec::new();
    for path in paths {
        let expected_contents = read_optional_file(&expected.join(path))?;
        let actual_contents = read_optional_file(&actual.join(path))?;
        if expected_contents != actual_contents {
            differing_paths.push(path.clone());
        }
    }
    Ok(differing_paths)
}

fn read_optional_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn verify_tree_status_paths(tree: &Path, owned_paths: &HashSet<PathBuf>) -> Result<()> {
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(tree)
        .output()?;
    if !status.status.success() {
        bail!("failed to inspect {}", tree.display());
    }

    for line in String::from_utf8_lossy(&status.stdout).lines() {
        let Some(path) = line.get(3..) else { continue };
        let path = Path::new(path.trim());
        if !owned_paths.contains(path)
            && !path.starts_with("Photon")
            && !path.starts_with("Patches")
            && path != Path::new("Meta/Photon")
            && path != Path::new("Build")
        {
            bail!(
                "{} contains unrepresented engine edits at {}",
                tree.display(),
                path.display()
            );
        }
    }
    Ok(())
}

/// Replace the patch materialization in existing generated trees without
/// removing their source worktrees or build directories.
pub(crate) fn refresh_generated_patch_series(
    repository: &Path,
    old_patches: &[Patch],
    new_patches: &[Patch],
) -> Result<()> {
    let trees = [build_tree(repository), edit_tree(repository)];
    let mut affected_paths = patches::patch_paths(repository, old_patches)?;
    affected_paths.extend(patches::patch_paths(repository, new_patches)?);

    // Validate every tree before changing either one. In particular, do not
    // reset an edit tree that contains changes outside the registered series.
    for tree in &trees {
        if tree.exists() {
            verify_tree_with_series(repository, tree, old_patches)?;
        }
    }

    for tree in trees {
        if tree.exists() {
            reconcile_tree_patch_series(repository, &tree, new_patches, &affected_paths)?;
        }
    }
    Ok(())
}

/// Restore the requested patch set after a failed in-place refresh.
pub(crate) fn restore_generated_patch_series(
    repository: &Path,
    patches: &[Patch],
    attempted_patches: &[Patch],
) -> Result<()> {
    let mut affected_paths = patches::patch_paths(repository, patches)?;
    affected_paths.extend(patches::patch_paths(repository, attempted_patches)?);
    for tree in [build_tree(repository), edit_tree(repository)] {
        if tree.exists() {
            reconcile_tree_patch_series(repository, &tree, patches, &affected_paths)?;
        }
    }
    Ok(())
}

/// Calculate desired patched contents from pristine source and touch only files
/// whose contents or permissions differ from the existing generated tree.
fn reconcile_tree_patch_series(
    repository: &Path,
    tree: &Path,
    patches: &[Patch],
    affected_paths: &HashSet<PathBuf>,
) -> Result<()> {
    let temporary = std::env::temp_dir().join(format!("photon-patch-reconcile-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)?;
    }
    fs::create_dir_all(&temporary)?;

    let result = (|| {
        let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
        let archive = temporary.join("base.tar");
        let mut upstream_paths = Vec::new();
        for path in affected_paths {
            let object = format!("{}:{}", upstream.revision, path.display());
            let exists = Command::new("git")
                .args(["cat-file", "-e", &object])
                .current_dir(repository)
                .stderr(std::process::Stdio::null())
                .status()?
                .success();
            if exists {
                upstream_paths.push(path);
            }
        }
        let base = temporary.join("base");
        fs::create_dir(&base)?;
        if !upstream_paths.is_empty() {
            let mut archive_command = Command::new("git");
            archive_command
                .args(["archive", "--format=tar", "-o"])
                .arg(&archive)
                .arg(&upstream.revision)
                .arg("--")
                .current_dir(repository);
            for path in upstream_paths {
                archive_command.arg(path);
            }
            git_success_command(&mut archive_command, "export patched paths from recorded upstream")?;

            let mut tar = Command::new("tar");
            tar.args(["-xf"]).arg(&archive).arg("-C").arg(&base);
            let status = tar.status().context("failed to extract upstream source files")?;
            if !status.success() {
                bail!("failed to extract upstream source files");
            }
            fs::remove_file(&archive)?;
        }

        for patch in patches {
            let patch_file = repository.join("Patches").join(&patch.file);
            let mut check = filtered_patch_command(&base, &patch_file, affected_paths, true);
            git_success_command(&mut check, &format!("check patch {}", patch.id))?;

            let mut apply = filtered_patch_command(&base, &patch_file, affected_paths, false);
            git_success_command(&mut apply, &format!("apply patch {}", patch.id))?;
        }

        for path in affected_paths {
            let current = tree.join(path);
            let desired = base.join(path);
            if desired.is_file() {
                let desired_contents = fs::read(&desired)?;
                let desired_permissions = fs::metadata(&desired)?.permissions();
                let current_matches = current.is_file()
                    && fs::read(&current).is_ok_and(|contents| contents == desired_contents)
                    && fs::metadata(&current)
                        .is_ok_and(|metadata| permissions_equal(&metadata.permissions(), &desired_permissions));
                if !current_matches {
                    replace_file(&current, &desired_contents, desired_permissions)?;
                }
            } else {
                match fs::symlink_metadata(&current) {
                    Ok(_) => fs::remove_file(current)?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }

        verify_tree_with_series(repository, tree, patches)
    })();

    fs::remove_dir_all(&temporary).context("failed to remove temporary patch reconciliation files")?;
    result
}

fn filtered_patch_command(directory: &Path, patch: &Path, paths: &HashSet<PathBuf>, check: bool) -> Command {
    let mut command = Command::new("git");
    command.arg("apply");
    if check {
        command.arg("--check");
    }
    for path in paths {
        command.arg(format!("--include={}", path.to_string_lossy()));
    }
    command.arg(patch).current_dir(directory);
    command
}

fn git_success_command(command: &mut Command, operation: &str) -> Result<()> {
    let status = command.status().with_context(|| format!("failed to {operation}"))?;
    if !status.success() {
        bail!("failed to {operation}");
    }
    Ok(())
}

fn replace_file(path: &Path, contents: &[u8], permissions: fs::Permissions) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut temporary_path = path.as_os_str().to_owned();
    temporary_path.push(format!(".photon-{}.tmp", std::process::id()));
    let temporary_path = PathBuf::from(temporary_path);
    fs::write(&temporary_path, contents)?;
    fs::set_permissions(&temporary_path, permissions)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary_path, path)?;
    Ok(())
}

#[cfg(unix)]
fn permissions_equal(left: &fs::Permissions, right: &fs::Permissions) -> bool {
    use std::os::unix::fs::PermissionsExt;
    left.mode() == right.mode()
}

#[cfg(not(unix))]
fn permissions_equal(left: &fs::Permissions, right: &fs::Permissions) -> bool {
    left.readonly() == right.readonly()
}

pub(crate) fn discard_build_tree(repository: &Path) -> Result<()> {
    let tree = build_tree(repository);
    if !tree.exists() {
        remove_alias_if_dangling(&repository.join(BUILD_TREE))?;
        return Ok(());
    }
    verify_tree(repository, &tree)?;
    park_build_directory(repository, &tree)?;
    let tree_argument = path_arg(&tree, repository)?;
    git_success(
        repository,
        &["worktree", "remove", "--force", &tree_argument],
        "refresh generated build source",
    )?;
    remove_alias_if_dangling(&repository.join(BUILD_TREE))
}

fn ensure_tree(repository: &Path, tree: &Path, include_build_link: bool) -> Result<()> {
    let alias = tree_alias(repository, tree)?;
    if tree.exists() {
        if tree == edit_tree(repository) {
            verify_edit_tree_path(repository, tree)?;
        } else {
            verify_tree(repository, tree).with_context(|| {
                format!("{} already exists but is not the exact registered materialization; preserve or inspect it before cleaning", tree.display())
            })?;
        }
        if include_build_link {
            ensure_build_directory(repository, tree)?;
        }
        ensure_alias(&alias, tree)?;
        return Ok(());
    }

    if alias.exists() && !fs::symlink_metadata(&alias)?.file_type().is_symlink() {
        if tree == build_tree(repository) {
            verify_tree(repository, &alias)?;
        } else {
            verify_edit_tree_path(repository, &alias)?;
        }
        if let Some(parent) = tree.parent() {
            fs::create_dir_all(parent)?;
        }
        let old_path = path_arg(&alias, repository)?;
        let new_path = path_arg(tree, repository)?;
        git_success(
            repository,
            &["worktree", "move", &old_path, &new_path],
            "move generated source outside the canonical checkout",
        )?;
        if tree == build_tree(repository) {
            ensure_build_directory(repository, tree)?;
            verify_tree(repository, tree)?;
        } else if include_build_link {
            ensure_build_directory(repository, tree)?;
        }
        ensure_alias(&alias, tree)?;
        return Ok(());
    }
    if fs::symlink_metadata(&alias).is_ok() {
        let target = fs::read_link(&alias)?;
        if target != tree {
            bail!(
                "{} already exists and does not point to the managed worktree",
                alias.display()
            );
        }
        bail!("{} points to a missing generated worktree", alias.display());
    }

    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    patches::validate_recorded_base(repository, &upstream.revision)?;
    if let Some(parent) = tree.parent() {
        fs::create_dir_all(parent)?;
    }
    let tree_argument = path_arg(tree, repository)?;
    git_success(
        repository,
        &["worktree", "add", "--detach", &tree_argument, &upstream.revision],
        "create pristine Ladybird worktree",
    )?;

    let setup = (|| {
        link_dir(&repository.join("Photon"), &tree.join("Photon"))?;
        link_dir(&repository.join("Patches"), &tree.join("Patches"))?;
        fs::create_dir_all(tree.join("Meta"))?;
        link_dir(&repository.join("Meta/Photon"), &tree.join("Meta/Photon"))?;
        if include_build_link {
            ensure_build_directory(repository, tree)?;
        }

        let patches = patches::series(repository)?;
        for patch in &patches {
            let path = repository.join("Patches").join(&patch.file);
            let patch_argument = path_arg(&path, repository)?;
            git_success(
                tree,
                &["apply", &patch_argument],
                &format!("materialize patch {}", patch.id),
            )?;
        }
        verify_tree(repository, tree)?;
        restore_source_mtimes(tree)?;
        Ok(())
    })();

    if let Err(error) = setup {
        let _ = git_success(
            repository,
            &["worktree", "remove", "--force", &tree_argument],
            "remove incomplete generated engine worktree",
        );
        return Err(error);
    }
    ensure_alias(&alias, tree)
}

fn ensure_build_directory(repository: &Path, tree: &Path) -> Result<()> {
    let persistent = persistent_build_directory(repository, tree)?;
    let build = tree.join("Build");
    match fs::symlink_metadata(&build) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(&build)?;
            fs::remove_file(&build)?;
            if target == persistent {
                // The target already is the parked artifact directory.
            } else if target.exists() {
                if persistent.exists() {
                    bail!(
                        "both {} and {} contain build data",
                        target.display(),
                        persistent.display()
                    );
                }
                if let Some(parent) = persistent.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(target, &persistent)?;
            }
        }
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if persistent.exists() {
        fs::rename(&persistent, &build)
            .with_context(|| format!("failed to restore build directory from {}", persistent.display()))?;
    } else {
        fs::create_dir_all(&build)?;
    }
    Ok(())
}

fn park_build_directory(repository: &Path, tree: &Path) -> Result<()> {
    let persistent = persistent_build_directory(repository, tree)?;
    let build = tree.join("Build");
    let metadata = match fs::symlink_metadata(&build) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if let Some(parent) = persistent.parent() {
        fs::create_dir_all(parent)?;
    }
    if !metadata.file_type().is_symlink() {
        save_source_mtimes(tree, &build)?;
    }
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(&build)?;
        fs::remove_file(&build)?;
        if target == persistent {
            return Ok(());
        }
        if persistent.exists() {
            bail!("cannot preserve build output: {} already exists", persistent.display());
        }
        if target.exists() {
            fs::rename(target, &persistent)?;
        }
    } else {
        if persistent.exists() {
            bail!("cannot preserve build output: {} already exists", persistent.display());
        }
        fs::rename(&build, &persistent)?;
    }
    Ok(())
}

// Keep source timestamps alongside parked build artifacts. Git worktree recreation
// gives every checked out file a fresh timestamp, which would make Ninja rebuild
// the entire engine despite reusing the previous objects.
fn save_source_mtimes(tree: &Path, build: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(tree)
        .output()?;
    if !output.status.success() {
        bail!("failed to list engine source files for timestamp preservation");
    }
    let diff = Command::new("git")
        .args(["diff", "--binary", "HEAD"])
        .current_dir(tree)
        .output()?;
    if !diff.status.success() {
        bail!("failed to record generated engine source state");
    }
    let mut manifest = fs::File::create(build.join(".photon-source-mtimes"))?;
    let mut state = fs::File::create(build.join(".photon-source-state"))?;
    state.write_all(git_output(tree, &["rev-parse", "HEAD"])?.as_bytes())?;
    state.write_all(b"\n")?;
    state.write_all(&diff.stdout)?;
    for entry in output.stdout.split(|byte| *byte == 0).filter(|entry| !entry.is_empty()) {
        let relative = std::str::from_utf8(entry)?;
        let path = tree.join(relative);
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() {
            continue;
        }
        let modified = match metadata.modified()?.duration_since(UNIX_EPOCH) {
            Ok(value) => value,
            Err(_) => Duration::ZERO,
        };
        writeln!(
            manifest,
            "{}\t{}\t{}",
            modified.as_secs(),
            modified.subsec_nanos(),
            relative
        )?;
    }
    Ok(())
}

fn restore_source_mtimes(tree: &Path) -> Result<()> {
    let build = tree.join("Build");
    let manifest_path = build.join(".photon-source-mtimes");
    let state_path = build.join(".photon-source-state");
    if !manifest_path.is_file() || !state_path.is_file() {
        return Ok(());
    }
    let current_diff = Command::new("git")
        .args(["diff", "--binary", "HEAD"])
        .current_dir(tree)
        .output()?;
    if !current_diff.status.success() {
        bail!("failed to inspect generated engine source state");
    }
    let mut current_state = git_output(tree, &["rev-parse", "HEAD"])?.into_bytes();
    current_state.push(b'\n');
    current_state.extend(current_diff.stdout);
    if fs::read(&state_path)? != current_state {
        fs::remove_file(manifest_path)?;
        fs::remove_file(state_path)?;
        return Ok(());
    }
    for line in fs::read_to_string(&manifest_path)?.lines() {
        let Some((seconds, rest)) = line.split_once('\t') else {
            continue;
        };
        let Some((nanos, relative)) = rest.split_once('\t') else {
            continue;
        };
        let path = tree.join(relative);
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_file()) {
            let file = fs::File::open(path)?;
            let time = UNIX_EPOCH + Duration::new(seconds.parse()?, nanos.parse()?);
            file.set_times(fs::FileTimes::new().set_modified(time))?;
        }
    }
    fs::remove_file(manifest_path)?;
    fs::remove_file(state_path)?;
    Ok(())
}

fn persistent_build_directory(repository: &Path, tree: &Path) -> Result<PathBuf> {
    let name = if tree == build_tree(repository) {
        "PhotonBuild"
    } else if tree == edit_tree(repository) {
        "PhotonEdit"
    } else {
        bail!("unknown generated engine tree {}", tree.display());
    };
    Ok(persistent_build_root(repository).join(name))
}

fn persistent_build_root(repository: &Path) -> PathBuf {
    let name = repository
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photon");
    repository
        .parent()
        .unwrap_or(repository)
        .join(format!(".{name}-photon-build"))
}

fn worktree_root(repository: &Path) -> PathBuf {
    let name = repository
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photon");
    repository
        .parent()
        .unwrap_or(repository)
        .join(format!(".{name}-photon-worktrees"))
}

fn tree_alias(repository: &Path, tree: &Path) -> Result<PathBuf> {
    if tree == build_tree(repository) {
        Ok(repository.join(BUILD_TREE))
    } else if tree == edit_tree(repository) {
        Ok(repository.join(EDIT_TREE))
    } else {
        bail!("unknown generated engine tree {}", tree.display())
    }
}

fn ensure_alias(alias: &Path, tree: &Path) -> Result<()> {
    if fs::symlink_metadata(alias).is_ok() {
        let current = fs::read_link(alias)
            .with_context(|| format!("{} exists and is not a worktree symlink", alias.display()))?;
        if current == tree {
            return Ok(());
        }
        bail!(
            "{} points to {}, expected {}",
            alias.display(),
            current.display(),
            tree.display()
        );
    }
    if let Some(parent) = alias.parent() {
        fs::create_dir_all(parent)?;
    }
    link_dir(tree, alias)
}

fn remove_alias_if_dangling(alias: &Path) -> Result<()> {
    match fs::symlink_metadata(alias) {
        Ok(metadata) if metadata.file_type().is_symlink() && !alias.exists() => {
            fs::remove_file(alias).map_err(Into::into)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn verify_tree(repository: &Path, tree: &Path) -> Result<()> {
    verify_tree_with_series(repository, tree, &patches::series(repository)?)
}

fn verify_tree_with_series(repository: &Path, tree: &Path, patches: &[Patch]) -> Result<()> {
    let upstream = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    let revision = git_output(tree, &["rev-parse", "HEAD"])?;
    if revision != upstream.revision {
        bail!(
            "{} is based on {}, expected recorded upstream {}",
            tree.display(),
            revision,
            upstream.revision
        );
    }
    patches::verify_materialized_files(tree, patches)?;

    let owned = patches::patch_paths(repository, patches)?;
    verify_tree_status_paths(tree, &owned)
}

fn build_tree(repository: &Path) -> PathBuf {
    worktree_root(repository).join("BuildSource")
}

fn path_arg(path: &Path, relative_to: &Path) -> Result<String> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        relative_to.join(path)
    };
    Ok(path.canonicalize().unwrap_or(path).to_string_lossy().into_owned())
}

fn git_output(directory: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).current_dir(directory).output()?;
    if !output.status.success() {
        bail!("git {} failed in {}", args.join(" "), directory.display());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn git_success(directory: &Path, args: &[&str], operation: &str) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(directory)
        .status()
        .with_context(|| format!("failed to {operation}"))?;
    if !status.success() {
        bail!("failed to {operation}");
    }
    Ok(())
}

#[cfg(unix)]
fn link_dir(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("failed to link {} to {}", link.display(), target.display()))
}

#[cfg(windows)]
fn link_dir(target: &Path, link: &Path) -> Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
        .with_context(|| format!("failed to link {} to {}", link.display(), target.display()))
}

#[cfg(not(any(unix, windows)))]
fn link_dir(_target: &Path, _link: &Path) -> Result<()> {
    bail!("generated engine trees require directory symlink support")
}
