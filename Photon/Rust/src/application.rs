// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: GPL-3.0-only

//! Photon application decisions that coordinate browser state and native effects.

use crate::browser::{BrowserState, ThemeMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppCommand<'a> {
    CreateTab,
    OpenSettings,
    SelectTab(u64),
    CloseTab(u64),
    ReorderTabs(&'a [u64]),
    SetTheme(ThemeMode),
    SetForceDarkPages(bool),
    SetDimOverlays(bool),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AppEffects {
    pub(crate) accepted: bool,
    pub(crate) state_changed: bool,
    pub(crate) tabs_changed: bool,
    pub(crate) active_tab_changed: bool,
    pub(crate) theme_changed: bool,
    pub(crate) force_dark_pages_changed: bool,
    pub(crate) created_tab_id: u64,
    pub(crate) removed_tab_id: u64,
}

pub(crate) struct PhotonApp {
    pub(crate) browser: BrowserState,
}

impl PhotonApp {
    pub(crate) fn new(browser: BrowserState) -> Self {
        Self { browser }
    }

    pub(crate) fn dispatch(&mut self, command: AppCommand<'_>) -> AppEffects {
        let old_active_tab_id = self.browser.active_tab_id();
        let mut effects = AppEffects::default();

        match command {
            AppCommand::CreateTab => {
                effects.created_tab_id = self.browser.create_tab();
                effects.accepted = true;
                effects.tabs_changed = true;
            }
            AppCommand::OpenSettings => {
                effects.created_tab_id = self.browser.open_settings();
                effects.accepted = true;
                effects.tabs_changed = true;
            }
            AppCommand::SelectTab(tab_id) => {
                effects.accepted = self.browser.select_tab(tab_id);
            }
            AppCommand::CloseTab(tab_id) => {
                effects.accepted = self.browser.close_tab(tab_id);
                if effects.accepted {
                    effects.tabs_changed = true;
                    if self.browser.contains_tab(tab_id) {
                        // Closing the final tab replaces its contents without removing its view.
                        effects.state_changed = true;
                    } else {
                        effects.removed_tab_id = tab_id;
                    }
                }
            }
            AppCommand::ReorderTabs(tab_ids) => {
                effects.accepted = self.browser.reorder_tabs(tab_ids);
                effects.tabs_changed = effects.accepted;
            }
            AppCommand::SetTheme(mode) => {
                effects.theme_changed = self.browser.set_theme_mode(mode);
                effects.accepted = effects.theme_changed;
                effects.state_changed = effects.theme_changed;
            }
            AppCommand::SetForceDarkPages(enabled) => {
                effects.force_dark_pages_changed = self.browser.set_force_dark_pages(enabled);
                effects.accepted = effects.force_dark_pages_changed;
                effects.state_changed = effects.force_dark_pages_changed;
            }
            AppCommand::SetDimOverlays(enabled) => {
                effects.state_changed = self.browser.set_dim_overlays(enabled);
                effects.accepted = effects.state_changed;
            }
        }

        effects.active_tab_changed = old_active_tab_id != self.browser.active_tab_id()
            || matches!(command, AppCommand::CloseTab(_)) && effects.removed_tab_id == 0 && effects.accepted;
        effects.state_changed |= effects.tabs_changed || effects.active_tab_changed;
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::{AppCommand, PhotonApp};
    use crate::browser::BrowserState;

    #[test]
    fn closing_active_tab_returns_native_view_effects() {
        let mut app = PhotonApp::new(BrowserState::initial());
        let first_tab_id = app.browser.active_tab_id();
        let created = app.dispatch(AppCommand::CreateTab);

        let effects = app.dispatch(AppCommand::CloseTab(created.created_tab_id));

        assert!(effects.accepted);
        assert_eq!(effects.removed_tab_id, created.created_tab_id);
        assert!(effects.active_tab_changed);
        assert_eq!(app.browser.active_tab_id(), first_tab_id);
    }

    #[test]
    fn closing_last_tab_keeps_native_view_and_resets_its_state() {
        let mut app = PhotonApp::new(BrowserState::initial());
        let tab_id = app.browser.active_tab_id();

        let effects = app.dispatch(AppCommand::CloseTab(tab_id));

        assert!(effects.accepted);
        assert_eq!(effects.removed_tab_id, 0);
        assert!(effects.active_tab_changed);
        assert!(effects.state_changed);
        assert_eq!(app.browser.active_tab_id(), tab_id);
    }
}
