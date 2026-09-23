// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: BSD-2-Clause

const NEW_TAB_URL: &str = "photon://newtab";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BrowserCommand {
    Navigate(String),
    Reload,
    Back,
    Forward,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NavigationCapabilities {
    pub(crate) can_go_back: bool,
    pub(crate) can_go_forward: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrowserTab {
    id: u64,
    url: String,
    title: String,
    favicon_url: Option<String>,
    loading: bool,
    navigation: NavigationCapabilities,
}

impl BrowserTab {
    fn new_tab(id: u64) -> Self {
        Self {
            id,
            url: NEW_TAB_URL.to_owned(),
            title: "New Tab".to_owned(),
            favicon_url: None,
            loading: false,
            navigation: NavigationCapabilities::default(),
        }
    }

    fn internal_page(&self) -> Option<&'static str> {
        match self.url.as_str() {
            "photon://newtab" => Some("new-tab"),
            "photon://settings" => Some("settings"),
            _ => None,
        }
    }
}

/// Rust owns browser tab metadata; each tab keeps its own Ladybird page view in the Qt adapter.
pub(crate) struct BrowserState {
    tabs: Vec<BrowserTab>,
    active_tab_id: u64,
    next_tab_id: u64,
    theme_mode: ThemeMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl BrowserState {
    pub(crate) fn initial() -> Self {
        Self {
            tabs: vec![BrowserTab::new_tab(1)],
            active_tab_id: 1,
            next_tab_id: 2,
            theme_mode: ThemeMode::System,
        }
    }

    fn active(&self) -> &BrowserTab {
        self.tab(self.active_tab_id).expect("active tab must exist")
    }

    fn tab(&self, id: u64) -> Option<&BrowserTab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    fn tab_mut(&mut self, id: u64) -> Option<&mut BrowserTab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    pub(crate) fn url(&self) -> &str {
        &self.active().url
    }

    pub(crate) fn title(&self) -> &str {
        &self.active().title
    }

    pub(crate) fn loading(&self) -> bool {
        self.active().loading
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.active().navigation.can_go_back && !self.is_internal_page()
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.active().navigation.can_go_forward && !self.is_internal_page()
    }

    pub(crate) fn active_tab_id(&self) -> u64 {
        self.active_tab_id
    }

    pub(crate) fn tab_ids(&self) -> Vec<u64> {
        self.tabs.iter().map(|tab| tab.id).collect()
    }

    pub(crate) fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) fn adjacent_tab_id(&self, previous: bool) -> u64 {
        let Some(current) = self.tabs.iter().position(|tab| tab.id == self.active_tab_id) else {
            return 0;
        };
        let offset = if previous { self.tabs.len() - 1 } else { 1 };
        self.tabs[(current + offset) % self.tabs.len()].id
    }

    pub(crate) fn theme_mode(&self) -> ThemeMode {
        self.theme_mode
    }

    pub(crate) fn set_theme_mode(&mut self, mode: ThemeMode) -> bool {
        if self.theme_mode == mode {
            return false;
        }
        self.theme_mode = mode;
        true
    }

    pub(crate) fn is_internal_page(&self) -> bool {
        self.active().internal_page().is_some()
    }

    pub(crate) fn set_url(&mut self, tab_id: u64, url: &str) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };
        let mut changed = replace_if_changed(&mut tab.url, url);
        if let Some(page) = tab.internal_page() {
            let title = if page == "settings" { "Settings" } else { "New Tab" };
            changed |= replace_if_changed(&mut tab.title, title);
            tab.loading = false;
        }
        changed
    }

    pub(crate) fn set_title(&mut self, tab_id: u64, title: &str) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };
        if tab.internal_page().is_some() {
            return false;
        }
        replace_if_changed(&mut tab.title, title)
    }

    pub(crate) fn set_favicon(&mut self, tab_id: u64, favicon_url: Option<&str>) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };
        let favicon_url = if tab.internal_page().is_some() {
            None
        } else {
            favicon_url.map(str::to_owned)
        };
        if tab.favicon_url == favicon_url {
            return false;
        }
        tab.favicon_url = favicon_url;
        true
    }

    pub(crate) fn set_loading(&mut self, tab_id: u64, loading: bool) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };
        let loading = loading && tab.internal_page().is_none();
        if tab.loading == loading {
            return false;
        }
        tab.loading = loading;
        true
    }

    pub(crate) fn set_navigation_capabilities(&mut self, tab_id: u64, navigation: NavigationCapabilities) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };
        let navigation = if tab.internal_page().is_some() {
            NavigationCapabilities::default()
        } else {
            navigation
        };
        if tab.navigation == navigation {
            return false;
        }
        tab.navigation = navigation;
        true
    }

    pub(crate) fn command_for_navigation(&self, input: &str) -> Option<BrowserCommand> {
        let url = normalize_url_input(input)?;
        Some(BrowserCommand::Navigate(url))
    }

    pub(crate) fn begin_navigation(&mut self, tab_id: u64, url: &str) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };
        tab.url = url.to_owned();
        tab.favicon_url = None;
        if let Some(page) = tab.internal_page() {
            tab.title = if page == "settings" { "Settings" } else { "New Tab" }.to_owned();
            tab.loading = false;
            tab.navigation = NavigationCapabilities::default();
        } else {
            tab.title.clear();
            tab.loading = true;
        }
        true
    }

    pub(crate) fn reload_command(&self) -> BrowserCommand {
        BrowserCommand::Reload
    }

    pub(crate) fn back_command(&self) -> Option<BrowserCommand> {
        self.can_go_back().then_some(BrowserCommand::Back)
    }

    pub(crate) fn forward_command(&self) -> Option<BrowserCommand> {
        self.can_go_forward().then_some(BrowserCommand::Forward)
    }

    pub(crate) fn create_tab(&mut self) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(BrowserTab::new_tab(id));
        self.active_tab_id = id;
        id
    }

    pub(crate) fn open_settings(&mut self) -> u64 {
        let tab_id = self.create_tab();
        self.begin_navigation(tab_id, "photon://settings");
        tab_id
    }

    pub(crate) fn select_tab(&mut self, tab_id: u64) -> bool {
        if self.tab(tab_id).is_none() || self.active_tab_id == tab_id {
            return false;
        }
        self.active_tab_id = tab_id;
        true
    }

    pub(crate) fn close_tab(&mut self, tab_id: u64) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return false;
        };
        if self.tabs.len() == 1 {
            self.tabs[0] = BrowserTab::new_tab(tab_id);
            self.active_tab_id = tab_id;
            return true;
        }
        self.tabs.remove(index);
        if self.active_tab_id == tab_id {
            self.active_tab_id = self.tabs[index.min(self.tabs.len() - 1)].id;
        }
        true
    }

    pub(crate) fn reorder_tabs(&mut self, ids: &[u64]) -> bool {
        if ids.len() != self.tabs.len() {
            return false;
        }
        let mut reordered = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(index) = self.tabs.iter().position(|tab| tab.id == *id) else {
                return false;
            };
            if reordered.iter().any(|tab: &BrowserTab| tab.id == *id) {
                return false;
            }
            reordered.push(self.tabs[index].clone());
        }
        self.tabs = reordered;
        true
    }

    pub(crate) fn snapshot_json(&self) -> String {
        let tabs = self
            .tabs
            .iter()
            .map(|tab| {
                format!(
                    "{{\"id\":\"tab-{}\",\"url\":{},\"title\":{},\"faviconUrl\":{},\"internalPage\":{},\"loading\":{},\"canGoBack\":{},\"canGoForward\":{}}}",
                    tab.id,
                    json_string(&tab.url),
                    json_string(&tab.title),
                    tab.favicon_url.as_deref().map_or("null".to_owned(), json_string),
                    tab.internal_page().map_or("null".to_owned(), json_string),
                    tab.loading,
                    tab.navigation.can_go_back && tab.internal_page().is_none(),
                    tab.navigation.can_go_forward && tab.internal_page().is_none(),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let theme_mode = match self.theme_mode {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        };
        format!(
            "{{\"tabs\":[{tabs}],\"activeTabId\":\"tab-{}\",\"themeMode\":\"{theme_mode}\"}}",
            self.active_tab_id
        )
    }
}

pub(crate) fn normalize_url_input(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    if input.contains("://")
        || ["about:", "data:", "file:", "view-source:"]
            .iter()
            .any(|prefix| input.starts_with(prefix))
    {
        return Some(input.to_owned());
    }

    Some(format!("https://{input}"))
}

fn replace_if_changed(current: &mut String, replacement: &str) -> bool {
    if current == replacement {
        return false;
    }
    replacement.clone_into(current);
    true
}

fn json_string(value: &str) -> String {
    let mut json = String::with_capacity(value.len() + 2);
    json.push('"');
    for character in value.chars() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write;
                let _ = write!(json, "\\u{:04x}", character as u32);
            }
            character => json.push(character),
        }
    }
    json.push('"');
    json
}

#[cfg(test)]
mod tests {
    use super::{BrowserCommand, BrowserState, NavigationCapabilities, ThemeMode, normalize_url_input};

    #[test]
    fn initial_window_opens_a_new_tab_page() {
        let state = BrowserState::initial();
        assert_eq!(state.url(), "photon://newtab");
        assert_eq!(state.title(), "New Tab");
        assert!(state.is_internal_page());
    }

    #[test]
    fn normalizes_hostname_input_to_https() {
        assert_eq!(
            normalize_url_input("example.com"),
            Some("https://example.com".to_owned())
        );
    }

    #[test]
    fn preserves_explicit_schemes_and_internal_pages() {
        assert_eq!(
            normalize_url_input("https://example.com"),
            Some("https://example.com".to_owned())
        );
        assert_eq!(
            normalize_url_input("photon://settings"),
            Some("photon://settings".to_owned())
        );
    }

    #[test]
    fn trims_input_and_rejects_empty_submissions() {
        assert_eq!(
            normalize_url_input("  example.com  "),
            Some("https://example.com".to_owned())
        );
        assert_eq!(normalize_url_input("   "), None);
    }

    #[test]
    fn engine_events_update_the_requested_tab() {
        let mut state = BrowserState::initial();
        let id = state.create_tab();
        state.begin_navigation(id, "https://example.org");
        state.set_title(id, "Example Organization");
        state.set_navigation_capabilities(
            id,
            NavigationCapabilities {
                can_go_back: true,
                can_go_forward: false,
            },
        );
        state.select_tab(1);
        assert_eq!(state.url(), "photon://newtab");
        state.select_tab(id);
        assert_eq!(state.title(), "Example Organization");
        assert!(state.can_go_back());
    }

    #[test]
    fn navigation_commands_update_rust_state_before_the_native_load() {
        let mut state = BrowserState::initial();
        assert_eq!(
            state.command_for_navigation("example.com"),
            Some(BrowserCommand::Navigate("https://example.com".to_owned()))
        );
        assert_eq!(state.url(), "photon://newtab");
        assert!(state.begin_navigation(1, "https://example.com"));
        assert_eq!(state.url(), "https://example.com");
        assert!(state.loading());
    }

    #[test]
    fn internal_pages_have_titles_and_disable_page_history() {
        let mut state = BrowserState::initial();
        assert!(state.begin_navigation(1, "photon://settings"));
        assert_eq!(state.title(), "Settings");
        assert!(!state.loading());
        state.set_navigation_capabilities(
            1,
            NavigationCapabilities {
                can_go_back: true,
                can_go_forward: true,
            },
        );
        assert!(!state.can_go_back());
        assert!(!state.can_go_forward());
    }

    #[test]
    fn closing_the_last_tab_reuses_it_as_a_new_tab() {
        let mut state = BrowserState::initial();
        assert!(state.close_tab(1));
        assert_eq!(state.tab_count(), 1);
        assert_eq!(state.url(), "photon://newtab");
    }

    #[test]
    fn opening_settings_creates_an_active_settings_tab() {
        let mut state = BrowserState::initial();
        let settings_tab_id = state.open_settings();
        assert_eq!(state.active_tab_id(), settings_tab_id);
        assert_eq!(state.url(), "photon://settings");
        assert_eq!(state.title(), "Settings");
        assert!(state.is_internal_page());
    }

    #[test]
    fn snapshot_json_includes_tabs_and_active_identity() {
        let state = BrowserState::initial();
        assert_eq!(
            state.snapshot_json(),
            "{\"tabs\":[{\"id\":\"tab-1\",\"url\":\"photon://newtab\",\"title\":\"New Tab\",\"faviconUrl\":null,\"internalPage\":\"new-tab\",\"loading\":false,\"canGoBack\":false,\"canGoForward\":false}],\"activeTabId\":\"tab-1\",\"themeMode\":\"system\"}"
        );
    }

    #[test]
    fn theme_setting_is_rust_owned() {
        let mut state = BrowserState::initial();
        assert!(state.set_theme_mode(ThemeMode::Dark));
        assert_eq!(state.theme_mode(), ThemeMode::Dark);
        assert!(!state.set_theme_mode(ThemeMode::Dark));
    }
}
