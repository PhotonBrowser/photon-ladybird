use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use console::style;

use crate::{metadata, ui};

pub fn status(repository: &Path) -> Result<i32> {
    let metadata = metadata::read_upstream(&repository.join("Meta/Photon/upstream.toml"))?;
    let remotes = git(repository, &["remote"])?;
    if !remotes.lines().any(|remote| remote == "upstream") {
        bail!("missing upstream remote; expected {}", metadata.repository);
    }

    let (upstream_ref, upstream_head) = match git(repository, &["rev-parse", "upstream/HEAD"]) {
        Ok(head) => ("upstream/HEAD", head),
        Err(_) => ("upstream/master", git(repository, &["rev-parse", "upstream/master"])?),
    };
    let behind = git(
        repository,
        &[
            "rev-list",
            "--count",
            &format!("{}..{upstream_head}", metadata.revision),
        ],
    )?;
    let ahead = git(
        repository,
        &[
            "rev-list",
            "--count",
            &format!("{upstream_head}..{}", metadata.revision),
        ],
    )?;
    let dirty = !git(repository, &["status", "--porcelain"])?.is_empty();

    ui::header("Photon", Some("Upstream"));
    ui::kv("Base", ui::sha(&metadata.revision));
    ui::hint(format!("Full base: {}", metadata.revision));
    ui::kv("Upstream", format!("{} ({upstream_ref})", ui::sha(&upstream_head)));
    let relationship = if behind == "0" && ahead == "0" {
        style("in sync").green().to_string()
    } else {
        format!("{behind} behind · {ahead} ahead")
    };
    ui::kv("Relationship", relationship);
    ui::kv(
        "Working tree",
        if dirty {
            style("modified").yellow().to_string()
        } else {
            style("clean").green().to_string()
        },
    );
    Ok(0)
}

pub(crate) fn git(repository: &Path, arguments: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .with_context(|| format!("failed to run git {}", arguments.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
