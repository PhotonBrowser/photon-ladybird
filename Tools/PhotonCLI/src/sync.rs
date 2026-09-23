// SPDX-License-Identifier: GPL-3.0-only
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{metadata, patches, ui, upstream};

const UPSTREAM_TOML: &str = "Meta/Photon/upstream.toml";
const TRACKING_REFS: [&str; 3] = ["upstream/HEAD", "upstream/master", "upstream/main"];

pub fn run(repository: &Path, fetch_only: bool, no_fetch: bool) -> Result<i32> {
    if fetch_only && no_fetch {
        bail!("--fetch-only and --no-fetch are mutually exclusive");
    }

    let recorded = metadata::read_upstream(&repository.join(UPSTREAM_TOML))?;
    let remotes = upstream::git(repository, &["remote"])?;
    if !remotes.lines().any(|remote| remote == "upstream") {
        bail!("missing upstream remote; expected {}", recorded.repository);
    }

    let branch = upstream::git(repository, &["branch", "--show-current"])?;
    if branch.is_empty() {
        bail!("not on a branch; check out a topic branch before syncing");
    }

    ui::header("Photon", Some("Sync upstream"));
    ui::kv("Branch", branch);
    ui::kv("Base", ui::sha(&recorded.revision));

    if !no_fetch {
        ui::step("Fetching upstream...");
        let fetched = Command::new("git")
            .args(["fetch", "upstream"])
            .current_dir(repository)
            .status()
            .context("failed to run git fetch upstream")?;
        if !fetched.success() {
            bail!("git fetch upstream failed");
        }
    } else {
        ui::hint("Skipped fetch (--no-fetch); using the local upstream tracking ref.");
    }

    let (target_ref, target_sha) = resolve_target(repository)?;
    ui::kv("Upstream", format!("{} ({target_ref})", ui::sha(&target_sha)));

    // A failed patch application leaves canonical Ladybird files pristine while
    // the contributor refreshes only Patches/. Let sync finish without a commit.
    if is_ancestor(repository, &target_sha, "HEAD")? && patches::is_patch_refresh_worktree(repository, &target_sha)? {
        check_series_on_revision(repository, &target_sha)?;
        ui::ok("Refreshed series", "registered patches apply to the merged upstream");
        return record_revision(repository, &recorded.revision, &target_sha);
    }

    validate_sync_worktree(repository, &target_sha)?;
    let series_check = check_series_on_revision(repository, &target_sha);

    if let Err(error) = series_check {
        if fetch_only {
            return Err(error);
        }

        if is_ancestor(repository, &target_sha, "HEAD")? {
            return Err(error.context("the registered patch series does not apply to this already-merged upstream"));
        }

        ui::step(format!(
            "Merging {target_ref} so the conflicting patch can be refreshed..."
        ));
        crate::engine::clean(repository)?;
        let merged = Command::new("git")
            .args(["merge", "--no-edit", &target_ref])
            .current_dir(repository)
            .status()
            .context("failed to run git merge")?;
        if !merged.success() {
            ui::failure("Merge stopped with conflicts.");
            ui::hint("Resolve them manually while the Ladybird patch series is unapplied, then update the patches.");
            ui::hint("Nothing was reset, discarded, or pushed.");
            return Ok(1);
        }

        ui::ok("Merged", ui::sha(&target_sha));
        ui::failure("The registered patch series does not apply to the merged upstream.");
        ui::hint("Refresh the affected files in Patches/ladybird and Patches/series.toml, then rerun `./photon sync`.");
        ui::hint("The second sync accepts an unstaged patch-only refresh; no temporary commit is needed.");
        ui::hint("The recorded upstream base was not changed and canonical source remains pristine.");
        return Ok(1);
    }

    if is_ancestor(repository, &target_sha, "HEAD")? {
        ui::success(format!("Already in sync with {target_ref}."));
        return record_revision(repository, &recorded.revision, &target_sha);
    }

    let incoming = upstream::git(repository, &["rev-list", "--count", &format!("HEAD..{target_sha}")])?;
    let local_only = upstream::git(repository, &["rev-list", "--count", &format!("{target_sha}..HEAD")])?;
    ui::kv("Divergence", format!("{incoming} incoming · {local_only} local-only"));

    if fetch_only {
        ui::hint("Fetch-only mode; the working tree was not changed.");
        return Ok(0);
    }

    ui::step(format!("Merging {target_ref} (history is never rewritten)..."));
    crate::engine::clean(repository)?;
    let merged = Command::new("git")
        .args(["merge", "--no-edit", &target_ref])
        .current_dir(repository)
        .status()
        .context("failed to run git merge")?;
    if !merged.success() {
        ui::failure("Merge stopped with conflicts.");
        ui::hint("Resolve them manually, then run `./photon patches check` and rebuild.");
        ui::hint("Nothing was reset, discarded, or pushed.");
        return Ok(1);
    }

    let head = upstream::git(repository, &["rev-parse", "HEAD"])?;
    ui::ok("Merged", ui::sha(&head));

    check_series_on_revision(repository, &head).context("Photon patch series does not apply to the merged checkout")?;

    record_revision(repository, &recorded.revision, &target_sha)
}

fn record_revision(repository: &Path, recorded: &str, target_sha: &str) -> Result<i32> {
    patches::validate_recorded_base(repository, target_sha)?;
    if recorded == target_sha {
        ui::kv("Recorded base", format!("{} (already current)", ui::sha(target_sha)));
        patches::ensure_materialized(repository)?;
        return Ok(0);
    }

    if !patches::is_pristine_at_revision(repository, target_sha)?
        && !patches::is_patch_refresh_worktree(repository, target_sha)?
    {
        bail!(
            "Ladybird checkout is not pristine at upstream {target_sha}, or contains changes beyond a patch-only refresh"
        );
    }

    crate::engine::clean(repository)?;

    let path = repository.join(UPSTREAM_TOML);
    let source = fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let updated = with_updated_revision(&source, target_sha)?;
    fs::write(&path, updated).with_context(|| format!("failed to update {}", path.display()))?;

    ui::ok("Recorded base", format!("{} in {UPSTREAM_TOML}", ui::sha(target_sha)));
    patches::ensure_materialized(repository)?;
    Ok(0)
}

fn validate_sync_worktree(repository: &Path, target_sha: &str) -> Result<()> {
    match patches::validate_sync_worktree(repository) {
        Ok(()) => Ok(()),
        Err(original_error) => {
            if patches::is_pristine_at_revision(repository, target_sha)? {
                Ok(())
            } else {
                Err(original_error)
            }
        }
    }
}

fn check_series_on_revision(repository: &Path, revision: &str) -> Result<()> {
    let series = patches::series(repository)?;
    let temporary = std::env::temp_dir().join(format!("photon-sync-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)?;
    }
    fs::create_dir(&temporary)?;
    let checked = patches::check_in(repository, &temporary, revision, &series);
    fs::remove_dir_all(&temporary).context("failed to remove temporary patch checkout")?;
    checked
        .context("Photon patch series does not apply to this revision; resolve and update the patches before syncing")
}

fn with_updated_revision(source: &str, revision: &str) -> Result<String> {
    let mut found = false;
    let mut rewritten: Vec<String> = Vec::new();
    for line in source.lines() {
        let is_revision = line.split_once('=').is_some_and(|(key, _)| key.trim() == "revision");
        if is_revision {
            rewritten.push(format!("revision = \"{revision}\""));
            found = true;
        } else {
            rewritten.push(line.to_owned());
        }
    }
    if !found {
        bail!("{UPSTREAM_TOML} has no revision entry to update");
    }
    let mut output = rewritten.join("\n");
    if source.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

fn resolve_target(repository: &Path) -> Result<(String, String)> {
    for candidate in TRACKING_REFS {
        if let Ok(sha) = upstream::git(repository, &["rev-parse", candidate]) {
            return Ok((candidate.to_owned(), sha));
        }
    }
    bail!("could not resolve an upstream tracking ref (tried upstream/HEAD, upstream/master, upstream/main)");
}

fn is_ancestor(repository: &Path, ancestor: &str, descendant: &str) -> Result<bool> {
    let status = Command::new("git")
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .current_dir(repository)
        .status()
        .context("failed to run git merge-base")?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("git merge-base --is-ancestor {ancestor} {descendant} failed unexpectedly"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_update_preserves_repository_line() {
        let source = "repository = \"https://github.com/LadybirdBrowser/ladybird.git\"\nrevision = \"abc123\"\n";
        let updated = with_updated_revision(source, "def456").unwrap();

        assert!(updated.contains("repository = \"https://github.com/LadybirdBrowser/ladybird.git\""));
        assert!(updated.contains("revision = \"def456\""));
        assert!(!updated.contains("abc123"));
    }

    #[test]
    fn revision_update_keeps_trailing_newline() {
        let updated = with_updated_revision("revision = \"abc\"\n", "def").unwrap();

        assert!(updated.ends_with('\n'));
    }

    #[test]
    fn revision_update_fails_without_a_revision_entry() {
        let source = "repository = \"https://example.test/repo.git\"\n";

        assert!(with_updated_revision(source, "def").is_err());
    }
}
