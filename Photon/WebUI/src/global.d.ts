// SPDX-License-Identifier: GPL-3.0-only
import type { BrowserSnapshot, PhotonApi } from "./types";

declare global {
    interface Window {
        photon: PhotonApi;
        __photonInitialState?: BrowserSnapshot;
        __photonPlatform?: "macos" | "other";
    }
}
