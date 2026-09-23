/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct PhotonUtf8 {
    uint8_t const* data;
    size_t len;
} PhotonUtf8;

typedef struct PhotonBrowserState PhotonBrowserState;

typedef enum PhotonBrowserCommandKind {
    PhotonBrowserCommandKind_None,
    PhotonBrowserCommandKind_Navigate,
    PhotonBrowserCommandKind_Reload,
    PhotonBrowserCommandKind_Back,
    PhotonBrowserCommandKind_Forward,
} PhotonBrowserCommandKind;

typedef struct PhotonBrowserCommand {
    PhotonBrowserCommandKind kind;
    PhotonUtf8 argument;
} PhotonBrowserCommand;

PhotonBrowserState* photon_browser_state_new(void);
void photon_browser_state_free(PhotonBrowserState* state);

// Returned strings borrow state storage and remain valid only until its next mutation.
PhotonUtf8 photon_browser_url(PhotonBrowserState const* state);
PhotonUtf8 photon_browser_title(PhotonBrowserState const* state);
uint8_t photon_browser_loading(PhotonBrowserState const* state);
uint8_t photon_browser_can_go_back(PhotonBrowserState const* state);
uint8_t photon_browser_can_go_forward(PhotonBrowserState const* state);
uint64_t photon_browser_active_tab_id(PhotonBrowserState const* state);
size_t photon_browser_tab_count(PhotonBrowserState const* state);
uint64_t photon_browser_tab_id_at(PhotonBrowserState const* state, size_t index);
uint64_t photon_browser_adjacent_tab_id(PhotonBrowserState const* state, uint8_t previous);
uint8_t photon_browser_is_internal_page(PhotonBrowserState const* state);
uint8_t photon_browser_theme_mode(PhotonBrowserState const* state);
uint8_t photon_browser_set_theme_mode(PhotonBrowserState* state, uint8_t mode);
PhotonUtf8 photon_browser_tabs_json(PhotonBrowserState* state);
uint64_t photon_browser_create_tab(PhotonBrowserState* state);
uint64_t photon_browser_open_settings(PhotonBrowserState* state);
uint8_t photon_browser_select_tab(PhotonBrowserState* state, uint64_t tab_id);
uint8_t photon_browser_close_tab(PhotonBrowserState* state, uint64_t tab_id);
uint8_t photon_browser_reorder_tabs(PhotonBrowserState* state, uint64_t const* ids, size_t len);

uint8_t photon_browser_set_url(PhotonBrowserState* state, uint64_t tab_id, uint8_t const* data, size_t len);
uint8_t photon_browser_set_title(PhotonBrowserState* state, uint64_t tab_id, uint8_t const* data, size_t len);
uint8_t photon_browser_set_favicon(PhotonBrowserState* state, uint64_t tab_id, uint8_t const* data, size_t len);
uint8_t photon_browser_set_loading(PhotonBrowserState* state, uint64_t tab_id, uint8_t loading);
uint8_t photon_browser_set_navigation_capabilities(PhotonBrowserState* state, uint64_t tab_id, uint8_t can_go_back, uint8_t can_go_forward);
uint8_t photon_browser_begin_navigation(PhotonBrowserState* state, uint64_t tab_id, uint8_t const* data, size_t len);

// A command argument borrows state storage and is consumed synchronously by the adapter.
PhotonBrowserCommand photon_browser_prepare_navigate(PhotonBrowserState* state, uint8_t const* data, size_t len);
PhotonBrowserCommand photon_browser_prepare_reload(PhotonBrowserState* state);
PhotonBrowserCommand photon_browser_prepare_back(PhotonBrowserState* state);
PhotonBrowserCommand photon_browser_prepare_forward(PhotonBrowserState* state);

#ifdef __cplusplus
}
#endif
