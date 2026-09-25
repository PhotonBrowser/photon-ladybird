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

typedef struct PhotonAppCommand {
    uint32_t kind;
    uint64_t value;
    uint64_t const* ids;
    size_t ids_len;
    PhotonUtf8 argument;
} PhotonAppCommand;

typedef struct PhotonPageObservation {
    uint32_t kind; // 1 URL, 2 title, 3 loading, 4 history capabilities, 5 favicon data URL.
    uint64_t tab_id;
    PhotonUtf8 text;
    uint8_t first;
    uint8_t second;
} PhotonPageObservation;

typedef struct PhotonAppEffects {
    uint8_t accepted;
    uint8_t state_changed;
    uint8_t tabs_changed;
    uint8_t active_tab_changed;
    uint8_t theme_changed;
    uint8_t force_dark_pages_changed;
    uint64_t created_tab_id;
    uint64_t removed_tab_id;
    uint64_t requested_close_tab_id;
    uint32_t navigation_kind;
    uint64_t navigation_tab_id;
    PhotonUtf8 navigation_target;
    uint8_t focus_address;
} PhotonAppEffects;

enum PhotonAppCommandKind {
    PhotonAppCommandKind_CreateTab = 1,
    PhotonAppCommandKind_OpenSettings = 2,
    PhotonAppCommandKind_SelectTab = 3,
    PhotonAppCommandKind_CloseTab = 4,
    PhotonAppCommandKind_ReorderTabs = 5,
    PhotonAppCommandKind_SetTheme = 6,
    PhotonAppCommandKind_SetDimOverlays = 7,
    PhotonAppCommandKind_SetForceDarkPages = 8,
    PhotonAppCommandKind_Navigate = 9,
    PhotonAppCommandKind_Reload = 10,
    PhotonAppCommandKind_Back = 11,
    PhotonAppCommandKind_Forward = 12,
    PhotonAppCommandKind_CloseActiveTab = 13,
    PhotonAppCommandKind_SelectPreviousTab = 14,
    PhotonAppCommandKind_SelectNextTab = 15,
    PhotonAppCommandKind_FocusAddress = 16,
    PhotonAppCommandKind_PageClosed = 17,
    PhotonAppCommandKind_SetWindowTintOpacity = 18,
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
uint8_t photon_browser_is_internal_page(PhotonBrowserState const* state);
uint8_t photon_browser_theme_mode(PhotonBrowserState const* state);
uint8_t photon_browser_force_dark_pages(PhotonBrowserState const* state);
PhotonUtf8 photon_browser_tabs_json(PhotonBrowserState* state);
PhotonAppEffects photon_app_dispatch(PhotonBrowserState* state, PhotonAppCommand command);
uint8_t photon_app_observe_page(PhotonBrowserState* state, PhotonPageObservation observation);

#ifdef __cplusplus
}
#endif
