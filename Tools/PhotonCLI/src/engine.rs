// SPDX-License-Identifier: GPL-3.0-only
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        let tree_argument = path_arg(&tree, repository)?;
        git_success(
            repository,
            &["worktree", "remove", "--force", &tree_argument],
            &format!("remove generated engine tree {}", tree.display()),
        )?;
        remove_alias_if_dangling(&alias)?;
    }
    ui::header("Photon", Some("Engine clean"));
    ui::ok("Generated source", "removed; build artifacts remain in Build/");
    Ok(0)
}

pub(crate) fn ensure_build_tree(repository: &Path) -> Result<PathBuf> {
    let editing = edit_tree(repository);
    if editing.exists() {
        verify_edit_tree(repository)?;
        ensure_build_link(repository, &editing)?;
        return Ok(editing);
    }
    let tree = build_tree(repository);
    ensure_tree(repository, &tree, true)?;
    Ok(tree)
}

pub(crate) fn edit_tree(repository: &Path) -> PathBuf {
    worktree_root(repository).join("Edit")
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
    let revision = git_output(&tree, &["rev-parse", "HEAD"])?;
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

pub(crate) fn discard_build_tree(repository: &Path) -> Result<()> {
    let tree = build_tree(repository);
    if !tree.exists() {
        remove_alias_if_dangling(&repository.join(BUILD_TREE))?;
        return Ok(());
    }
    verify_tree(repository, &tree)?;
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
            ensure_build_link(repository, tree)?;
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
            ensure_build_link(repository, tree)?;
            verify_tree(repository, tree)?;
        } else if include_build_link {
            ensure_build_link(repository, tree)?;
        }
        ensure_alias(&alias, tree)?;
        return Ok(());
    }
    if fs::symlink_metadata(&alias).is_ok() {
        let target = fs::read_link(&alias)?;
        if target != tree {
            bail!("{} already exists and does not point to the managed worktree", alias.display());
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
            // Ladybird's presets put binaries under <source>/Build. Share the
            // repository Build directory, including existing user artifacts.
            ensure_build_link(repository, tree)?;
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
        verify_tree(repository, tree)
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

fn ensure_build_link(repository: &Path, tree: &Path) -> Result<()> {
    let build_root = persistent_build_root(repository);
    let build_directory = build_root.join(if tree == build_tree(repository) {
        "PhotonBuild"
    } else {
        "PhotonEdit"
    });
    fs::create_dir_all(&build_directory)?;
    let link = tree.join("Build");
    match fs::symlink_metadata(&link) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let current = fs::read_link(&link)?;
            if current == build_directory {
                return Ok(());
            }
            fs::remove_file(&link)?;
        }
        Ok(_) => bail!("{} exists and is not the managed build-directory symlink", link.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    link_dir(&build_directory, &link)
}

fn persistent_build_root(repository: &Path) -> PathBuf {
    let name = repository.file_name().and_then(|name| name.to_str()).unwrap_or("photon");
    repository
        .parent()
        .unwrap_or(repository)
        .join(format!(".{name}-photon-build"))
}

fn worktree_root(repository: &Path) -> PathBuf {
    let name = repository.file_name().and_then(|name| name.to_str()).unwrap_or("photon");
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
        bail!("{} points to {}, expected {}", alias.display(), current.display(), tree.display());
    }
    if let Some(parent) = alias.parent() {
        fs::create_dir_all(parent)?;
    }
    link_dir(tree, alias)
}

fn remove_alias_if_dangling(alias: &Path) -> Result<()> {
    match fs::symlink_metadata(alias) {
        Ok(metadata) if metadata.file_type().is_symlink() && !alias.exists() => fs::remove_file(alias).map_err(Into::into),
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

    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(tree)
        .output()?;
    if !status.status.success() {
        bail!("failed to inspect {}", tree.display());
    }
    let paths = String::from_utf8_lossy(&status.stdout);
    let owned = patches::patch_paths(repository, patches)?;
    for line in paths.lines() {
        let Some(path) = line.get(3..) else { continue };
        let path = Path::new(path.trim());
        if !owned.contains(path)
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

fn build_tree(repository: &Path) -> PathBuf {
    worktree_root(repository).join("BuildSource")
}

fn path_arg(path: &Path, relative_to: &Path) -> Result<String> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        relative_to.join(path)
    };
    Ok(path
        .canonicalize()
        .or_else(|_| Ok::<PathBuf, std::io::Error>(path))?
        .to_string_lossy()
        .into_owned())
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
