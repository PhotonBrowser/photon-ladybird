// SPDX-License-Identifier: GPL-3.0-only
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

#[derive(Debug, PartialEq, Eq)]
pub struct UpstreamMetadata {
    pub repository: String,
    pub revision: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Patch {
    pub id: String,
    pub file: String,
    pub description: String,
    pub area: String,
}

pub fn read_upstream(path: &Path) -> Result<UpstreamMetadata> {
    let source = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(UpstreamMetadata {
        repository: required_value(&source, "repository", path)?,
        revision: required_value(&source, "revision", path)?,
    })
}

pub fn read_patch_series(path: &Path) -> Result<Vec<Patch>> {
    let source = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if value(&source, "version").as_deref() != Some("1") {
        bail!("unsupported patch manifest version in {}", path.display());
    }

    source
        .split("[[patches]]")
        .skip(1)
        .map(|entry| {
            Ok(Patch {
                id: required_value(entry, "id", path)?,
                file: required_value(entry, "file", path)?,
                description: required_value(entry, "description", path)?,
                area: required_value(entry, "area", path)?,
            })
        })
        .collect()
}

fn required_value(source: &str, key: &str, path: &Path) -> Result<String> {
    value(source, key).with_context(|| format!("missing {key} in {}", path.display()))
}

fn value(source: &str, key: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let (candidate, raw_value) = line.split_once('=')?;
        if candidate.trim() != key {
            return None;
        }
        Some(raw_value.trim().trim_matches('"').to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upstream_metadata() {
        let source = "repository = \"https://example.test/repo.git\"\nrevision = \"abc123\"";

        assert_eq!(value(source, "revision").as_deref(), Some("abc123"));
    }

    #[test]
    fn parses_empty_patch_series() {
        let entries = "version = 1\n".split("[[patches]]").skip(1).count();

        assert_eq!(entries, 0);
    }
}
