/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
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

typedef struct PhotonAppCommand {
    uint32_t kind;
    uint64_t value;
    uint64_t const* ids;
    size_t ids_len;
} PhotonAppCommand;

typedef struct PhotonAppEffects {
    uint8_t accepted;
    uint8_t state_changed;
    uint8_t tabs_changed;
    uint8_t active_tab_changed;
    uint8_t theme_changed;
    uint64_t created_tab_id;
    uint64_t removed_tab_id;
} PhotonAppEffects;

enum PhotonAppCommandKind {
    PhotonAppCommandKind_CreateTab = 1,
    PhotonAppCommandKind_OpenSettings = 2,
    PhotonAppCommandKind_SelectTab = 3,
    PhotonAppCommandKind_CloseTab = 4,
    PhotonAppCommandKind_ReorderTabs = 5,
    PhotonAppCommandKind_SetTheme = 6,
    PhotonAppCommandKind_SetDimOverlays = 7,
};

PhotonBrowserState* photon_browser_state_new(uint8_t const* config_path, size_t config_path_len);
void photon_browser_state_free(PhotonBrowserState* state);

// Returned strings borrow state storage and remain valid only until its next mutation.
PhotonUtf8 photon_browser_url(PhotonBrowserState const* state);
PhotonUtf8 photon_browser_title(PhotonBrowserState const* state);
uint8_t photon_browser_loading(PhotonBrowserState const* state);
uint8_t photon_browser_can_go_back(PhotonBrowserState const* state);
uint8_t photon_browser_can_go_forward(PhotonBrowserState const* state);
uint64_t photon_browser_active_tab_id(PhotonBrowserState const* state);
uint64_t photon_browser_adjacent_tab_id(PhotonBrowserState const* state, uint8_t previous);
uint8_t photon_browser_is_internal_page(PhotonBrowserState const* state);
uint8_t photon_browser_theme_mode(PhotonBrowserState const* state);
PhotonUtf8 photon_browser_tabs_json(PhotonBrowserState* state);
PhotonAppEffects photon_app_dispatch(PhotonBrowserState* state, PhotonAppCommand command);

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
