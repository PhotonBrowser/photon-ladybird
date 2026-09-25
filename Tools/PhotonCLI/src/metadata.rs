// SPDX-License-Identifier: GPL-3.0-only
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

#[derive(Debug, PartialEq, Eq)]
pub struct UpstreamMetadata {
    pub repository: String,
    pub revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    pub id: String,
    pub file: String,
    pub description: String,
    pub area: String,
    pub enabled: bool,
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
            let enabled = match value(entry, "enabled").as_deref() {
                None | Some("true") => true,
                Some("false") => false,
                Some(value) => bail!("invalid enabled value `{value}` in {}", path.display()),
            };
            Ok(Patch {
                id: required_value(entry, "id", path)?,
                file: required_value(entry, "file", path)?,
                description: required_value(entry, "description", path)?,
                area: required_value(entry, "area", path)?,
                enabled,
            })
        })
        .collect()
}

/// Update one patch's enabled flag without reformatting the rest of the manifest.
pub fn set_patch_enabled(path: &Path, id: &str, enabled: bool) -> Result<bool> {
    let source = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let patches = read_patch_series(path)?;
    if !patches.iter().any(|patch| patch.id == id) {
        bail!("unknown patch ID `{id}`");
    }

    let mut lines = source.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut section_start = None;
    let mut target_range = None;
    for (index, line) in lines.iter().enumerate() {
        if line.trim() == "[[patches]]" {
            if let Some(start) = section_start.replace(index) {
                if value(&lines[start..index].join("\n"), "id").as_deref() == Some(id) {
                    target_range = Some((start, index));
                    break;
                }
            }
        }
    }
    if target_range.is_none() {
        if let Some(start) = section_start {
            if value(&lines[start..].join("\n"), "id").as_deref() == Some(id) {
                target_range = Some((start, lines.len()));
            }
        }
    }
    let (start, end) = target_range.context("patch entry not found in manifest")?;
    let current_enabled = patches.iter().find(|patch| patch.id == id).unwrap().enabled;
    if current_enabled == enabled {
        return Ok(false);
    }

    let enabled_line = (start + 1..end).find(|index| lines[*index].trim_start().starts_with("enabled"));
    if enabled {
        if let Some(index) = enabled_line {
            lines.remove(index);
        }
    } else if let Some(index) = enabled_line {
        lines[index] = "enabled = false".to_owned();
    } else {
        let id_line = (start + 1..end)
            .find(|index| lines[*index].trim_start().starts_with("id"))
            .context("patch entry has no ID line")?;
        lines.insert(id_line + 1, "enabled = false".to_owned());
    }

    let mut updated = lines.join("\n");
    if source.ends_with('\n') {
        updated.push('\n');
    }
    fs::write(path, updated).with_context(|| format!("failed to update {}", path.display()))?;
    Ok(true)
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

    #[test]
    fn patch_enabled_defaults_to_true_and_can_be_disabled() {
        let path = std::env::temp_dir().join(format!("photon-patch-series-{}.toml", std::process::id()));
        let source = "version = 1\n\n[[patches]]\nid = \"0001\"\nfile = \"a.patch\"\ndescription = \"A\"\narea = \"Test\"\n\n[[patches]]\nid = \"0002\"\nfile = \"b.patch\"\ndescription = \"B\"\narea = \"Test\"\n";
        fs::write(&path, source).unwrap();

        let parsed = read_patch_series(&path).unwrap();
        assert!(parsed.iter().all(|patch| patch.enabled));
        assert!(set_patch_enabled(&path, "0002", false).unwrap());
        let parsed = read_patch_series(&path).unwrap();
        assert!(parsed[0].enabled);
        assert!(!parsed[1].enabled);
        assert!(set_patch_enabled(&path, "0002", true).unwrap());
        assert!(read_patch_series(&path).unwrap().iter().all(|patch| patch.enabled));

        fs::remove_file(path).unwrap();
    }
}
