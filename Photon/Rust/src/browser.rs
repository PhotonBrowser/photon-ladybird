// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: BSD-2-Clause

const INITIAL_URL: &str = "https://example.com";

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

/// Photon-owned state for one attached Ladybird web-content surface.
pub(crate) struct BrowserState {
    url: String,
    title: String,
    loading: bool,
    navigation: NavigationCapabilities,
}

impl BrowserState {
    pub(crate) fn initial() -> Self {
        Self {
            url: INITIAL_URL.to_owned(),
            title: String::new(),
            loading: false,
            navigation: NavigationCapabilities::default(),
        }
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.navigation.can_go_back
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.navigation.can_go_forward
    }

    pub(crate) fn set_url(&mut self, url: &str) -> bool {
        replace_if_changed(&mut self.url, url)
    }

    pub(crate) fn set_title(&mut self, title: &str) -> bool {
        replace_if_changed(&mut self.title, title)
    }

    pub(crate) fn set_loading(&mut self, loading: bool) -> bool {
        if self.loading == loading {
            return false;
        }
        self.loading = loading;
        true
    }

    pub(crate) fn set_navigation_capabilities(&mut self, navigation: NavigationCapabilities) -> bool {
        if self.navigation == navigation {
            return false;
        }
        self.navigation = navigation;
        true
    }

    pub(crate) fn command_for_navigation(&self, input: &str) -> Option<BrowserCommand> {
        normalize_url_input(input).map(BrowserCommand::Navigate)
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

#[cfg(test)]
mod tests {
    use super::{BrowserCommand, BrowserState, NavigationCapabilities, normalize_url_input};

    #[test]
    fn initial_browser_state_uses_example_dot_com() {
        assert_eq!(BrowserState::initial().url(), "https://example.com");
    }

    #[test]
    fn normalizes_hostname_input_to_https() {
        assert_eq!(
            normalize_url_input("example.com"),
            Some("https://example.com".to_owned())
        );
    }

    #[test]
    fn preserves_explicit_http_and_https_schemes() {
        assert_eq!(
            normalize_url_input("https://example.com"),
            Some("https://example.com".to_owned())
        );
        assert_eq!(
            normalize_url_input("http://example.com"),
            Some("http://example.com".to_owned())
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
    fn engine_events_update_product_facing_state() {
        let mut state = BrowserState::initial();

        state.set_url("https://example.org/redirected");
        state.set_title("Example Organization");
        state.set_loading(true);
        state.set_navigation_capabilities(NavigationCapabilities {
            can_go_back: true,
            can_go_forward: false,
        });

        assert_eq!(state.url(), "https://example.org/redirected");
        assert_eq!(state.title(), "Example Organization");
        assert!(state.loading());
        assert!(state.can_go_back());
        assert!(!state.can_go_forward());
    }

    #[test]
    fn commands_are_typed_and_navigation_is_normalized() {
        let state = BrowserState::initial();

        assert_eq!(
            state.command_for_navigation("example.com"),
            Some(BrowserCommand::Navigate("https://example.com".to_owned()))
        );
        assert_eq!(state.command_for_navigation(""), None);
        assert_eq!(state.reload_command(), BrowserCommand::Reload);
    }

    #[test]
    fn history_commands_follow_engine_capabilities() {
        let mut state = BrowserState::initial();
        assert_eq!(state.back_command(), None);
        assert_eq!(state.forward_command(), None);

        state.set_navigation_capabilities(NavigationCapabilities {
            can_go_back: true,
            can_go_forward: true,
        });

        assert_eq!(state.back_command(), Some(BrowserCommand::Back));
        assert_eq!(state.forward_command(), Some(BrowserCommand::Forward));
    }
}
