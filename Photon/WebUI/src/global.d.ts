import type { BrowserSnapshot, PhotonApi } from "./types";

declare global {
    interface Window {
        photon: PhotonApi;
        __photonInitialState?: BrowserSnapshot;
    }
}
