// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: GPL-3.0-only

//! The only unsafe boundary between Photon Rust state and the native adapter.

use std::{slice, str};

use crate::application::{AppCommand, AppEffects, NativeNavigation, PageObservation, PhotonApp};
use crate::browser::{BrowserState, NavigationCapabilities, ThemeMode};

#[repr(C)]
pub struct PhotonUtf8 {
    pub data: *const u8,
    pub len: usize,
}

impl Default for PhotonUtf8 {
    fn default() -> Self {
        Self {
            data: std::ptr::null(),
            len: 0,
        }
    }
}

#[repr(C)]
pub struct PhotonAppCommand {
    pub kind: u32,
    pub value: u64,
    pub ids: *const u64,
    pub ids_len: usize,
    pub argument: PhotonUtf8,
}

#[repr(C)]
pub struct PhotonPageObservation {
    pub kind: u32,
    pub tab_id: u64,
    pub text: PhotonUtf8,
    pub first: u8,
    pub second: u8,
}

/// # Safety
/// `state` must be a live Photon state and `observation.text` must remain readable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_app_observe_page(
    state: *mut PhotonBrowserState,
    observation: PhotonPageObservation,
) -> u8 {
    let Some(state) = (unsafe { state.as_mut() }) else {
        return 0;
    };
    let event = match observation.kind {
        1 | 2 | 5 => {
            let Some(text) = (unsafe { input_utf8(observation.text.data, observation.text.len) }) else {
                return 0;
            };
            match observation.kind {
                1 => PageObservation::Url(text),
                2 => PageObservation::Title(text),
                _ => PageObservation::Favicon(text),
            }
        }
        3 => PageObservation::Loading(observation.first != 0),
        4 => PageObservation::Navigation(NavigationCapabilities {
            can_go_back: observation.first != 0,
            can_go_forward: observation.second != 0,
        }),
        _ => return 0,
    };
    state.app.observe_page(observation.tab_id, event).into()
}

#[repr(u32)]
enum PhotonAppCommandKind {
    CreateTab = 1,
    OpenSettings = 2,
    SelectTab = 3,
    CloseTab = 4,
    ReorderTabs = 5,
    SetTheme = 6,
    SetDimOverlays = 7,
    SetForceDarkPages = 8,
    Navigate = 9,
    Reload = 10,
    Back = 11,
    Forward = 12,
    CloseActiveTab = 13,
    SelectPreviousTab = 14,
    SelectNextTab = 15,
    FocusAddress = 16,
    PageClosed = 17,
}

impl TryFrom<u32> for PhotonAppCommandKind {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::CreateTab),
            2 => Ok(Self::OpenSettings),
            3 => Ok(Self::SelectTab),
            4 => Ok(Self::CloseTab),
            5 => Ok(Self::ReorderTabs),
            6 => Ok(Self::SetTheme),
            7 => Ok(Self::SetDimOverlays),
            8 => Ok(Self::SetForceDarkPages),
            9 => Ok(Self::Navigate),
            10 => Ok(Self::Reload),
            11 => Ok(Self::Back),
            12 => Ok(Self::Forward),
            13 => Ok(Self::CloseActiveTab),
            14 => Ok(Self::SelectPreviousTab),
            15 => Ok(Self::SelectNextTab),
            16 => Ok(Self::FocusAddress),
            17 => Ok(Self::PageClosed),
            _ => Err(()),
        }
    }
}

#[repr(C)]
#[derive(Default)]
pub struct PhotonAppEffects {
    pub accepted: u8,
    pub state_changed: u8,
    pub tabs_changed: u8,
    pub active_tab_changed: u8,
    pub theme_changed: u8,
    pub force_dark_pages_changed: u8,
    pub created_tab_id: u64,
    pub removed_tab_id: u64,
    pub requested_close_tab_id: u64,
    pub navigation_kind: u32,
    pub navigation_tab_id: u64,
    pub navigation_target: PhotonUtf8,
    pub focus_address: u8,
}

impl PhotonAppEffects {
    fn from_effects(effects: AppEffects, argument: &str) -> Self {
        let (navigation_kind, navigation_tab_id) = match effects.navigation {
            Some(NativeNavigation::Navigate { tab_id, .. }) => (1, tab_id),
            Some(NativeNavigation::Reload { tab_id }) => (2, tab_id),
            Some(NativeNavigation::Back { tab_id }) => (3, tab_id),
            Some(NativeNavigation::Forward { tab_id }) => (4, tab_id),
            None => (0, 0),
        };
        Self {
            accepted: effects.accepted.into(),
            state_changed: effects.state_changed.into(),
            tabs_changed: effects.tabs_changed.into(),
            active_tab_changed: effects.active_tab_changed.into(),
            theme_changed: effects.theme_changed.into(),
            force_dark_pages_changed: effects.force_dark_pages_changed.into(),
            created_tab_id: effects.created_tab_id,
            removed_tab_id: effects.removed_tab_id,
            requested_close_tab_id: effects.requested_close_tab_id,
            navigation_kind,
            navigation_tab_id,
            navigation_target: borrowed_utf8(Some(argument)),
            focus_address: effects.focus_address.into(),
        }
    }
}

pub struct PhotonBrowserState {
    app: PhotonApp,
    command_argument: String,
    snapshot_json: String,
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `config_path` must be null or point to `config_path_len` readable UTF-8 bytes for this call.
pub unsafe extern "C" fn photon_browser_state_new(
    config_path: *const u8,
    config_path_len: usize,
) -> *mut PhotonBrowserState {
    // SAFETY: Native code supplies a byte slice valid for this call.
    let config_path = unsafe { input_utf8(config_path, config_path_len) }
        .filter(|path| !path.is_empty())
        .map(std::path::Path::new);
    Box::into_raw(Box::new(PhotonBrowserState {
        app: PhotonApp::new(config_path.map_or_else(BrowserState::initial, BrowserState::initial_with_config)),
        command_argument: String::new(),
        snapshot_json: String::new(),
    }))
}

/// Dispatches Photon application operations and returns typed integration effects.
///
/// # Safety
/// `state` must be null or a live state returned by `photon_browser_state_new`. For reorder
/// commands, `command.ids` must point to `ids_len` readable `u64` values for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_app_dispatch(
    state: *mut PhotonBrowserState,
    command: PhotonAppCommand,
) -> PhotonAppEffects {
    // SAFETY: The caller guarantees the state pointer remains live for this call.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return PhotonAppEffects::default();
    };

    let Ok(kind) = PhotonAppCommandKind::try_from(command.kind) else {
        return PhotonAppEffects::default();
    };
    let command = match kind {
        PhotonAppCommandKind::CreateTab => AppCommand::CreateTab,
        PhotonAppCommandKind::OpenSettings => AppCommand::OpenSettings,
        PhotonAppCommandKind::SelectTab => AppCommand::SelectTab(command.value),
        PhotonAppCommandKind::CloseTab => AppCommand::CloseTab(command.value),
        PhotonAppCommandKind::PageClosed => AppCommand::PageClosed(command.value),
        PhotonAppCommandKind::ReorderTabs => {
            // SAFETY: The caller guarantees this buffer remains live for this call.
            let Some(ids) = (unsafe { input_ids(command.ids, command.ids_len) }) else {
                return PhotonAppEffects::default();
            };
            AppCommand::ReorderTabs(ids)
        }
        PhotonAppCommandKind::SetTheme => AppCommand::SetTheme(match command.value {
            0 => ThemeMode::System,
            1 => ThemeMode::Light,
            2 => ThemeMode::Dark,
            _ => return PhotonAppEffects::default(),
        }),
        PhotonAppCommandKind::SetDimOverlays => AppCommand::SetDimOverlays(match command.value {
            0 => false,
            1 => true,
            _ => return PhotonAppEffects::default(),
        }),
        PhotonAppCommandKind::SetForceDarkPages => AppCommand::SetForceDarkPages(match command.value {
            0 => false,
            1 => true,
            _ => return PhotonAppEffects::default(),
        }),
        PhotonAppCommandKind::Navigate => {
            let Some(input) = (unsafe { input_utf8(command.argument.data, command.argument.len) }) else {
                return PhotonAppEffects::default();
            };
            AppCommand::Navigate(input)
        }
        PhotonAppCommandKind::Reload => AppCommand::Reload,
        PhotonAppCommandKind::Back => AppCommand::Back,
        PhotonAppCommandKind::Forward => AppCommand::Forward,
        PhotonAppCommandKind::CloseActiveTab => AppCommand::CloseActiveTab,
        PhotonAppCommandKind::SelectPreviousTab => AppCommand::SelectAdjacentTab { previous: true },
        PhotonAppCommandKind::SelectNextTab => AppCommand::SelectAdjacentTab { previous: false },
        PhotonAppCommandKind::FocusAddress => AppCommand::FocusAddress,
    };

    let effects = state.app.dispatch(command);
    state.command_argument = match effects.navigation.as_ref() {
        Some(NativeNavigation::Navigate { target, .. }) => target.clone(),
        _ => String::new(),
    };
    PhotonAppEffects::from_effects(effects, &state.command_argument)
}

/// # Safety
///
/// `state` must be null or the live pointer returned by `photon_browser_state_new`, and it
/// must be passed to this function at most once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_state_free(state: *mut PhotonBrowserState) {
    if !state.is_null() {
        // SAFETY: The caller promises this is the unique live allocation returned by `new`.
        drop(unsafe { Box::from_raw(state) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_url(state: *const PhotonBrowserState) -> PhotonUtf8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    borrowed_utf8(unsafe { state.as_ref() }.map(|state| state.app.browser.url()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_title(state: *const PhotonBrowserState) -> PhotonUtf8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    borrowed_utf8(unsafe { state.as_ref() }.map(|state| state.app.browser.title()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_loading(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }
        .is_some_and(|state| state.app.browser.loading())
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_can_go_back(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }
        .is_some_and(|state| state.app.browser.can_go_back())
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_can_go_forward(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }
        .is_some_and(|state| state.app.browser.can_go_forward())
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_active_tab_id(state: *const PhotonBrowserState) -> u64 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }.map_or(0, |state| state.app.browser.active_tab_id())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_is_internal_page(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }
        .is_some_and(|state| state.app.browser.is_internal_page())
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_theme_mode(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    match unsafe { state.as_ref() }.map(|state| state.app.browser.theme_mode()) {
        Some(ThemeMode::Light) => 1,
        Some(ThemeMode::Dark) => 2,
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_force_dark_pages(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }
        .is_some_and(|state| state.app.browser.force_dark_pages())
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_tabs_json(state: *mut PhotonBrowserState) -> PhotonUtf8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return borrowed_utf8(None);
    };
    state.snapshot_json = state.app.browser.snapshot_json();
    borrowed_utf8(Some(&state.snapshot_json))
}

unsafe fn input_ids<'a>(ids: *const u64, len: usize) -> Option<&'a [u64]> {
    if len == 0 {
        return Some(&[]);
    }
    if ids.is_null() {
        return None;
    }
    // SAFETY: The caller promises `ids` addresses `len` readable u64 values for this call.
    Some(unsafe { slice::from_raw_parts(ids, len) })
}

unsafe fn input_utf8<'a>(data: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    if data.is_null() {
        return None;
    }
    // SAFETY: The caller promises that `data` addresses `len` readable bytes for this call.
    str::from_utf8(unsafe { slice::from_raw_parts(data, len) }).ok()
}

fn borrowed_utf8(value: Option<&str>) -> PhotonUtf8 {
    let bytes = value.unwrap_or_default().as_bytes();
    PhotonUtf8 {
        data: bytes.as_ptr(),
        len: bytes.len(),
    }
}
