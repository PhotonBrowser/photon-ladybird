// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::browser::ThemeMode;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct PhotonConfig {
    pub(crate) theme_mode: ThemeMode,
    pub(crate) force_dark_pages: bool,
    pub(crate) dim_overlays: bool,
}

impl Default for PhotonConfig {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            force_dark_pages: false,
            dim_overlays: false,
        }
    }
}

impl PhotonConfig {
    pub(crate) fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("Could not parse Photon config at {}: {error}", path.display());
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                config.save(path);
                config
            }
            Err(error) => {
                eprintln!("Could not read Photon config at {}: {error}", path.display());
                Self::default()
            }
        }
    }

    pub(crate) fn save(&self, path: &Path) {
        let result = toml::to_string_pretty(self)
            .map_err(std::io::Error::other)
            .and_then(|contents| {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(path, contents)
            });

        if let Err(error) = result {
            eprintln!("Could not save Photon config at {}: {error}", path.display());
        }
    }
}
