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

uint8_t photon_browser_set_url(PhotonBrowserState* state, uint8_t const* data, size_t len);
uint8_t photon_browser_set_title(PhotonBrowserState* state, uint8_t const* data, size_t len);
uint8_t photon_browser_set_loading(PhotonBrowserState* state, uint8_t loading);
uint8_t photon_browser_set_navigation_capabilities(PhotonBrowserState* state, uint8_t can_go_back, uint8_t can_go_forward);

// A command argument borrows state storage and is consumed synchronously by the adapter.
PhotonBrowserCommand photon_browser_prepare_navigate(PhotonBrowserState* state, uint8_t const* data, size_t len);
PhotonBrowserCommand photon_browser_prepare_reload(PhotonBrowserState* state);
PhotonBrowserCommand photon_browser_prepare_back(PhotonBrowserState* state);
PhotonBrowserCommand photon_browser_prepare_forward(PhotonBrowserState* state);

#ifdef __cplusplus
}
#endif
