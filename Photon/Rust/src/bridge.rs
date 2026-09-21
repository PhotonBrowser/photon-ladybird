// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: BSD-2-Clause

//! The only unsafe boundary between Photon Rust state and the native adapter.

use std::{slice, str};

use crate::browser::{BrowserCommand, BrowserState, NavigationCapabilities};

#[repr(C)]
pub struct PhotonUtf8 {
    pub data: *const u8,
    pub len: usize,
}

#[repr(C)]
pub enum PhotonBrowserCommandKind {
    None,
    Navigate,
    Reload,
    Back,
    Forward,
}

#[repr(C)]
pub struct PhotonBrowserCommand {
    pub kind: PhotonBrowserCommandKind,
    pub argument: PhotonUtf8,
}

pub struct PhotonBrowserState {
    browser: BrowserState,
    command_argument: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn photon_browser_state_new() -> *mut PhotonBrowserState {
    Box::into_raw(Box::new(PhotonBrowserState {
        browser: BrowserState::initial(),
        command_argument: String::new(),
    }))
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
    borrowed_utf8(unsafe { state.as_ref() }.map(|state| state.browser.url()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_title(state: *const PhotonBrowserState) -> PhotonUtf8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    borrowed_utf8(unsafe { state.as_ref() }.map(|state| state.browser.title()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_loading(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }
        .is_some_and(|state| state.browser.loading())
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_can_go_back(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }
        .is_some_and(|state| state.browser.can_go_back())
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_can_go_forward(state: *const PhotonBrowserState) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_ref() }
        .is_some_and(|state| state.browser.can_go_forward())
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_set_url(state: *mut PhotonBrowserState, data: *const u8, len: usize) -> u8 {
    // SAFETY: This function forwards its documented native pointer contract unchanged.
    unsafe { update_string(state, data, len, BrowserState::set_url) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_set_title(state: *mut PhotonBrowserState, data: *const u8, len: usize) -> u8 {
    // SAFETY: This function forwards its documented native pointer contract unchanged.
    unsafe { update_string(state, data, len, BrowserState::set_title) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_set_loading(state: *mut PhotonBrowserState, loading: u8) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_mut() }
        .is_some_and(|state| state.browser.set_loading(loading != 0))
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_set_navigation_capabilities(
    state: *mut PhotonBrowserState,
    can_go_back: u8,
    can_go_forward: u8,
) -> u8 {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    unsafe { state.as_mut() }
        .is_some_and(|state| {
            state.browser.set_navigation_capabilities(NavigationCapabilities {
                can_go_back: can_go_back != 0,
                can_go_forward: can_go_forward != 0,
            })
        })
        .into()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_prepare_navigate(
    state: *mut PhotonBrowserState,
    data: *const u8,
    len: usize,
) -> PhotonBrowserCommand {
    // SAFETY: Native code supplies a live state and an input buffer valid for this call.
    let (Some(state), Some(input)) = (unsafe { state.as_mut() }, unsafe { input_utf8(data, len) }) else {
        return no_command();
    };
    let Some(command) = state.browser.command_for_navigation(input) else {
        return no_command();
    };
    prepare_command(state, command)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_prepare_reload(state: *mut PhotonBrowserState) -> PhotonBrowserCommand {
    // SAFETY: Native code keeps the state alive for the duration of this call.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return no_command();
    };
    prepare_command(state, state.browser.reload_command())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_prepare_back(state: *mut PhotonBrowserState) -> PhotonBrowserCommand {
    // SAFETY: This function forwards its documented native pointer contract unchanged.
    unsafe { prepare_optional_history_command(state, BrowserState::back_command) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_prepare_forward(state: *mut PhotonBrowserState) -> PhotonBrowserCommand {
    // SAFETY: This function forwards its documented native pointer contract unchanged.
    unsafe { prepare_optional_history_command(state, BrowserState::forward_command) }
}

fn prepare_command(state: &mut PhotonBrowserState, command: BrowserCommand) -> PhotonBrowserCommand {
    let kind = match command {
        BrowserCommand::Navigate(url) => {
            state.command_argument = url;
            PhotonBrowserCommandKind::Navigate
        }
        BrowserCommand::Reload => PhotonBrowserCommandKind::Reload,
        BrowserCommand::Back => PhotonBrowserCommandKind::Back,
        BrowserCommand::Forward => PhotonBrowserCommandKind::Forward,
    };
    PhotonBrowserCommand {
        kind,
        argument: borrowed_utf8(Some(&state.command_argument)),
    }
}

unsafe fn prepare_optional_history_command(
    state: *mut PhotonBrowserState,
    command: fn(&BrowserState) -> Option<BrowserCommand>,
) -> PhotonBrowserCommand {
    // SAFETY: The caller forwards the native pointer whose lifetime covers this call.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return no_command();
    };
    command(&state.browser).map_or_else(no_command, |command| prepare_command(state, command))
}

unsafe fn update_string(
    state: *mut PhotonBrowserState,
    data: *const u8,
    len: usize,
    update: fn(&mut BrowserState, &str) -> bool,
) -> u8 {
    // SAFETY: The caller supplies a live state and an input buffer valid for this call.
    let (Some(state), Some(value)) = (unsafe { state.as_mut() }, unsafe { input_utf8(data, len) }) else {
        return 0;
    };
    update(&mut state.browser, value).into()
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

fn no_command() -> PhotonBrowserCommand {
    PhotonBrowserCommand {
        kind: PhotonBrowserCommandKind::None,
        argument: borrowed_utf8(None),
    }
}
