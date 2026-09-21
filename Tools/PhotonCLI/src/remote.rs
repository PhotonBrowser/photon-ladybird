use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

const ORIGIN_TOML: &str = "Meta/Photon/origin.toml";
const UPSTREAM_DOC: &str = "Documentation/Photon/Upstream.md";

pub fn status(repository: &Path) -> Result<i32> {
    let canonical = read_canonical(repository);
    let fetch = git_remote_url(repository, false)?;
    let push = git_remote_url(repository, true)?;

    println!("Canonical origin  {}", canonical.as_deref().unwrap_or("(not recorded)"));
    println!(
        "Origin fetch      {}",
        fetch.as_deref().unwrap_or("(missing)")
    );
    println!("Origin push       {}", push.as_deref().unwrap_or("(missing)"));

    match (&canonical, &fetch) {
        (Some(canonical), Some(fetch)) if same_repository(canonical, fetch) => {
            println!("Relationship      in sync");
            Ok(0)
        }
        (Some(_), Some(_)) => {
            println!("Relationship      diverged (run `photon remote set-origin <url>`)");
            Ok(1)
        }
        _ => {
            println!("Relationship      unknown");
            Ok(1)
        }
    }
}

pub fn set_origin(repository: &Path, value: Option<&str>, ssh: bool, https: bool) -> Result<i32> {
    if ssh && https {
        bail!("--ssh and --https are mutually exclusive");
    }

    let existing = git_remote_url(repository, false)?;
    let target = resolve_target(value, ssh, https, existing.as_deref())?;
    let canonical = to_https(&target);

    ensure_git_remote(repository, &existing, &target)?;
    update_origin_toml(repository, &canonical)?;
    update_upstream_doc(repository, &canonical)?;

    println!("Origin fetch/push  {target}");
    println!("Canonical record  {canonical}");
    println!("Updated           {ORIGIN_TOML}, {UPSTREAM_DOC}");
    Ok(0)
}

fn resolve_target(
    value: Option<&str>,
    ssh: bool,
    https: bool,
    existing: Option<&str>,
) -> Result<String> {
    if let Some(raw) = value {
        let raw = raw.trim();
        if raw.is_empty() {
            bail!("origin repository must not be empty");
        }
        let mut url = expand_shorthand(raw, existing)?;
        if ssh {
            url = to_ssh(&url);
        } else if https {
            url = to_https(&url);
        }
        Ok(ensure_git_suffix(&url))
    } else if ssh || https {
        let current = existing.context("no origin remote configured; pass a repository URL explicitly")?;
        let converted = if ssh { to_ssh(current) } else { to_https(current) };
        Ok(ensure_git_suffix(&converted))
    } else {
        bail!("pass a repository URL (or owner/repo), or use --ssh/--https to convert the current origin");
    }
}

fn expand_shorthand(raw: &str, existing: Option<&str>) -> Result<String> {
    if raw.contains("://") || raw.starts_with("git@") {
        return Ok(raw.to_owned());
    }
    if let Some(path) = raw.strip_prefix("github.com/") {
        return Ok(format!("https://github.com/{}", path.trim_matches('/')));
    }
    // Bare `owner/repo` shorthand. Keep the existing protocol when possible so
    // developers using SSH do not get silently switched to HTTPS.
    if is_owner_repo(raw) {
        let base = existing
            .map(|url| {
                if url.starts_with("git@") || url.starts_with("ssh://") {
                    to_ssh(&format!("https://github.com/{raw}"))
                } else {
                    format!("https://github.com/{raw}")
                }
            })
            .unwrap_or_else(|| format!("https://github.com/{raw}"));
        return Ok(base);
    }
    bail!("could not interpret {raw:?} as a Git URL or owner/repo shorthand");
}

fn is_owner_repo(value: &str) -> bool {
    let mut parts = value.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) => !owner.is_empty() && !repo.is_empty(),
        _ => false,
    }
}

pub(crate) fn to_https(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return format!("https://github.com/{}", rest.trim_matches('/'));
    }
    if let Some(rest) = url.strip_prefix("ssh://git@github.com/") {
        return format!("https://github.com/{}", rest.trim_matches('/'));
    }
    url.to_owned()
}

pub(crate) fn to_ssh(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        return format!("git@github.com:{}", rest.trim_matches('/'));
    }
    if let Some(rest) = url.strip_prefix("http://github.com/") {
        return format!("git@github.com:{}", rest.trim_matches('/'));
    }
    if let Some(rest) = url.strip_prefix("ssh://git@github.com/") {
        return format!("git@github.com:{}", rest.trim_matches('/'));
    }
    url.to_owned()
}

fn ensure_git_suffix(url: &str) -> String {
    if url.ends_with(".git") { url.to_owned() } else { format!("{url}.git") }
}

fn same_repository(first: &str, second: &str) -> bool {
    normalize_path(first) == normalize_path(second)
}

fn normalize_path(url: &str) -> String {
    let https = to_https(url);
    let without_scheme = https
        .strip_prefix("https://github.com/")
        .or_else(|| https.strip_prefix("http://github.com/"))
        .unwrap_or(&https);
    without_scheme
        .trim_matches('/')
        .trim_end_matches(".git")
        .to_ascii_lowercase()
}

fn read_canonical(repository: &Path) -> Option<String> {
    let source = fs::read_to_string(repository.join(ORIGIN_TOML)).ok()?;
    source.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if key.trim() == "repository" {
            Some(value.trim().trim_matches('"').to_owned())
        } else {
            None
        }
    })
}

fn git_remote_url(repository: &Path, push: bool) -> Result<Option<String>> {
    let mut command = Command::new("git");
    command.current_dir(repository).arg("remote").arg("get-url");
    if push {
        command.arg("--push");
    }
    command.arg("origin");
    let output = command.output().context("failed to inspect the origin remote")?;
    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
    } else {
        Ok(None)
    }
}

fn ensure_git_remote(repository: &Path, existing: &Option<String>, target: &str) -> Result<()> {
    let mut command = Command::new("git");
    command.current_dir(repository);
    if existing.is_some() {
        command.args(["remote", "set-url", "origin", target]);
    } else {
        command.args(["remote", "add", "origin", target]);
    }
    let status = command.status().context("failed to update the origin remote")?;
    if !status.success() {
        bail!("git failed to point origin at {target}");
    }
    Ok(())
}

fn update_origin_toml(repository: &Path, canonical: &str) -> Result<()> {
    let path = repository.join(ORIGIN_TOML);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&path, format!("repository = \"{canonical}\"\n"))
        .with_context(|| format!("failed to update {}", path.display()))?;
    Ok(())
}

fn update_upstream_doc(repository: &Path, canonical: &str) -> Result<()> {
    let path = repository.join(UPSTREAM_DOC);
    let Ok(source) = fs::read_to_string(&path) else {
        println!("Skipped           {UPSTREAM_DOC} (not present)");
        return Ok(());
    };
    let mut updated = false;
    let rewritten: Vec<String> = source
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("origin") {
                updated = true;
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                format!("{indent}origin    {canonical} (Photon repository)")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !updated {
        bail!("{UPSTREAM_DOC} has no origin entry to update");
    }
    let mut output = rewritten.join("\n");
    if source.ends_with('\n') {
        output.push('\n');
    }
    fs::write(&path, output).with_context(|| format!("failed to update {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_owner_repo_shorthand_to_https() {
        assert_eq!(
            expand_shorthand("PhotonBrowser/photon-ladybird", None).unwrap(),
            "https://github.com/PhotonBrowser/photon-ladybird"
        );
    }

    #[test]
    fn shorthand_preserves_ssh_when_origin_uses_ssh() {
        let expanded =
            expand_shorthand("PhotonBrowser/photon-ladybird", Some("git@github.com:OldOrg/old.git"))
                .unwrap();

        assert_eq!(expanded, "git@github.com:PhotonBrowser/photon-ladybird");
    }

    #[test]
    fn converts_between_ssh_and_https_forms() {
        assert_eq!(
            to_https("git@github.com:PhotonBrowser/photon-ladybird.git"),
            "https://github.com/PhotonBrowser/photon-ladybird.git"
        );
        assert_eq!(
            to_ssh("https://github.com/PhotonBrowser/photon-ladybird.git"),
            "git@github.com:PhotonBrowser/photon-ladybird.git"
        );
    }

    #[test]
    fn repository_comparison_ignores_protocol_case_and_suffix() {
        assert!(same_repository(
            "https://github.com/PhotonBrowser/photon-ladybird.git",
            "git@github.com:photonbrowser/photon-ladybird"
        ));
        assert!(!same_repository(
            "https://github.com/PhotonBrowser/photon-ladybird.git",
            "https://github.com/OtherOrg/photon-ladybird.git"
        ));
    }

    #[test]
    fn protocol_flags_convert_the_existing_origin() {
        let ssh = resolve_target(None, true, false, Some("https://github.com/PhotonBrowser/photon-ladybird.git"))
            .unwrap();
        let https = resolve_target(None, false, true, Some("git@github.com:PhotonBrowser/photon-ladybird.git"))
            .unwrap();

        assert_eq!(ssh, "git@github.com:PhotonBrowser/photon-ladybird.git");
        assert_eq!(https, "https://github.com/PhotonBrowser/photon-ladybird.git");
    }
}
