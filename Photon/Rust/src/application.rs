// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: GPL-3.0-only

//! Photon application decisions that coordinate browser state and native effects.

use crate::browser::{BrowserCommand, BrowserState, NavigationCapabilities, ThemeMode};

pub(crate) enum PageObservation<'a> {
    Url(&'a str),
    Title(&'a str),
    Loading(bool),
    Navigation(NavigationCapabilities),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NativeNavigation {
    Navigate { tab_id: u64, target: String },
    Reload { tab_id: u64 },
    Back { tab_id: u64 },
    Forward { tab_id: u64 },
}

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
    Navigate(&'a str),
    Reload,
    Back,
    Forward,
    CloseActiveTab,
    SelectAdjacentTab { previous: bool },
    FocusAddress,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct AppEffects {
    pub(crate) accepted: bool,
    pub(crate) state_changed: bool,
    pub(crate) tabs_changed: bool,
    pub(crate) active_tab_changed: bool,
    pub(crate) theme_changed: bool,
    pub(crate) force_dark_pages_changed: bool,
    pub(crate) created_tab_id: u64,
    pub(crate) removed_tab_id: u64,
    pub(crate) requested_close_tab_id: u64,
    pub(crate) navigation: Option<NativeNavigation>,
    pub(crate) focus_address: bool,
}

pub(crate) struct PhotonApp {
    pub(crate) browser: BrowserState,
}

impl PhotonApp {
    pub(crate) fn new(browser: BrowserState) -> Self {
        Self { browser }
    }

    pub(crate) fn observe_page(&mut self, tab_id: u64, observation: PageObservation<'_>) -> bool {
        match observation {
            PageObservation::Url(url) => self.browser.set_url(tab_id, url),
            PageObservation::Title(title) => self.browser.set_title(tab_id, title),
            PageObservation::Loading(loading) => self.browser.set_loading(tab_id, loading),
            PageObservation::Navigation(capabilities) => self.browser.set_navigation_capabilities(tab_id, capabilities),
        }
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
            AppCommand::Navigate(input) => {
                if let Some(BrowserCommand::Navigate(target)) = self.browser.command_for_navigation(input) {
                    let tab_id = self.browser.active_tab_id();
                    self.browser.begin_navigation(tab_id, &target);
                    effects.navigation =
                        (!self.browser.is_internal_page()).then_some(NativeNavigation::Navigate { tab_id, target });
                    effects.state_changed = true;
                    effects.accepted = true;
                }
            }
            AppCommand::Reload => {
                effects.accepted = true;
                if !self.browser.is_internal_page() {
                    effects.navigation = Some(NativeNavigation::Reload {
                        tab_id: self.browser.active_tab_id(),
                    });
                }
            }
            AppCommand::Back | AppCommand::Forward => {
                let tab_id = self.browser.active_tab_id();
                effects.navigation = match command {
                    AppCommand::Back if self.browser.can_go_back() => Some(NativeNavigation::Back { tab_id }),
                    AppCommand::Forward if self.browser.can_go_forward() => Some(NativeNavigation::Forward { tab_id }),
                    _ => None,
                };
                effects.accepted = effects.navigation.is_some();
            }
            AppCommand::CloseActiveTab => {
                effects.accepted = true;
                effects.requested_close_tab_id = self.browser.active_tab_id();
            }
            AppCommand::SelectAdjacentTab { previous } => {
                return self.dispatch(AppCommand::SelectTab(self.browser.adjacent_tab_id(previous)));
            }
            AppCommand::FocusAddress => {
                effects.accepted = true;
                effects.focus_address = true;
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
    use super::{AppCommand, NativeNavigation, PageObservation, PhotonApp};
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

    #[test]
    fn navigation_effect_targets_the_active_tab_and_internal_routes_stay_in_rust() {
        let mut app = PhotonApp::new(BrowserState::initial());
        let tab_id = app.dispatch(AppCommand::CreateTab).created_tab_id;
        let effect = app.dispatch(AppCommand::Navigate("example.com"));
        assert_eq!(
            effect.navigation,
            Some(NativeNavigation::Navigate {
                tab_id,
                target: "https://example.com".to_owned()
            })
        );
        assert!(effect.state_changed);

        let internal = app.dispatch(AppCommand::Navigate("photon://settings"));
        assert!(internal.accepted);
        assert!(internal.navigation.is_none());
        assert_eq!(app.browser.url(), "photon://settings");
    }

    #[test]
    fn history_and_adjacent_selection_use_rust_state() {
        let mut app = PhotonApp::new(BrowserState::initial());
        let first = app.browser.active_tab_id();
        let second = app.dispatch(AppCommand::CreateTab).created_tab_id;
        app.dispatch(AppCommand::Navigate("example.com"));
        assert!(app.observe_page(
            second,
            PageObservation::Navigation(crate::browser::NavigationCapabilities {
                can_go_back: true,
                can_go_forward: false
            })
        ));
        assert_eq!(
            app.dispatch(AppCommand::Back).navigation,
            Some(NativeNavigation::Back { tab_id: second })
        );
        assert_eq!(app.dispatch(AppCommand::Forward).navigation, None);
        app.dispatch(AppCommand::SelectAdjacentTab { previous: true });
        assert_eq!(app.browser.active_tab_id(), first);
        assert_eq!(app.dispatch(AppCommand::CloseActiveTab).requested_close_tab_id, first);
        assert_eq!(app.browser.active_tab_id(), first);
    }
}
