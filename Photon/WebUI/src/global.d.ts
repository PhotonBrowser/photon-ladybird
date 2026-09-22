import type { BrowserState, PhotonApi } from "./types";

declare global {
    interface Window {
        photon: PhotonApi;
        __photonInitialState?: BrowserState;
    }
}
