// SPDX-License-Identifier: GPL-3.0-only
import type { BrowserSnapshot, PhotonApi } from "./types";

declare global {
    interface Window {
        photon: PhotonApi;
        embedderMessaging?: {
            postMessage(type: string, payload: unknown): void;
            receiveMessages(): string;
        };
        __photonInitialState?: BrowserSnapshot;
        __photonPlatform?: "macos" | "other";
    }
}
