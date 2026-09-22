import { createRoot } from "react-dom/client";

import App from "./App";
import "./styles.css";
import type { BrowserState, PhotonApi } from "./types";

const initialStateFromDevServer = (): BrowserState | undefined => {
    const value = new URLSearchParams(window.location.search).get("photonInitialState");
    if (!value) return undefined;
    try {
        return JSON.parse(value) as BrowserState;
    } catch {
        return undefined;
    }
};

const initialState: BrowserState = window.__photonInitialState ??
    initialStateFromDevServer() ?? {
        url: "https://example.com",
        title: "Photon",
        loading: false,
        canGoBack: false,
        canGoForward: false,
    };

const command = (name: string, value?: string): void => {
    const query = value === undefined ? "" : `?value=${encodeURIComponent(value)}`;
    window.location.href = `photon-command://${name}${query}`;
};

const photon: PhotonApi = {
    navigation: {
        navigate: (url) => command("navigate", url),
        back: () => command("back"),
        forward: () => command("forward"),
        reload: () => command("reload"),
    },
    browser: { state: initialState },
};

window.photon = photon;

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Photon Web UI root element is missing");

createRoot(rootElement).render(<App api={photon} />);
