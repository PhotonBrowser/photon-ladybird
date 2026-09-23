// SPDX-License-Identifier: GPL-3.0-only
import { createRoot } from "react-dom/client";

import App from "./App";
import { createNavigationCommandTransport } from "./bridge/transport";
import "./styles.css";
import type { BrowserSnapshot, PhotonApi, ThemeMode } from "./types";

const searchParams = new URLSearchParams(window.location.search);

const developmentSnapshot: BrowserSnapshot = {
    tabs: [
        {
            id: "tab-1",
            url: "photon://newtab",
            title: "New Tab",
            faviconUrl: null,
            internalPage: "new-tab",
            loading: false,
            canGoBack: false,
            canGoForward: false,
        },
    ],
    activeTabId: "tab-1",
    themeMode: "system",
    forceDarkPages: false,
    dimOverlays: false,
};

const snapshotFromDevServer = (): BrowserSnapshot | undefined => {
    const value = searchParams.get("photonInitialState");
    if (!value) return undefined;
    try {
        return JSON.parse(value) as BrowserSnapshot;
    } catch {
        return undefined;
    }
};

let snapshot = window.__photonInitialState ?? snapshotFromDevServer() ?? developmentSnapshot;
const listeners = new Set<(state: BrowserSnapshot) => void>();
const nativeBridgeAvailable = window.__photonInitialState !== undefined || searchParams.has("photonInitialState");
const platform = window.__photonPlatform ?? (searchParams.get("photonPlatform") === "macos" ? "macos" : "other");

const commandTransport = createNavigationCommandTransport(nativeBridgeAvailable);

const parseThemeMode = (value: unknown): ThemeMode | undefined =>
    value === "system" || value === "light" || value === "dark" ? value : undefined;

const photon: PhotonApi = {
    navigation: {
        navigate: (url) => commandTransport.dispatch({ kind: "navigate", url }),
        back: () => commandTransport.dispatch({ kind: "back" }),
        forward: () => commandTransport.dispatch({ kind: "forward" }),
        reload: () => commandTransport.dispatch({ kind: "reload" }),
    },
    tabs: {
        create: () => commandTransport.dispatch({ kind: "new-tab" }),
        openSettings: () => commandTransport.dispatch({ kind: "open-settings" }),
        select: (tabId) => commandTransport.dispatch({ kind: "select-tab", tabId }),
        close: (tabId) => commandTransport.dispatch({ kind: "close-tab", tabId }),
        reorder: (tabIds) => commandTransport.dispatch({ kind: "reorder-tabs", tabIds }),
    },
    ui: {
        setCaptureRegion: (name, open) => {
            if (name === "site-info" || name === "browser-menu")
                commandTransport.dispatch({ kind: "capture", region: name, open });
        },
    },
    settings: {
        setTheme: (mode) => commandTransport.dispatch({ kind: "set-theme", mode }),
        setForceDarkPages: (enabled) => commandTransport.dispatch({ kind: "set-force-dark-pages", enabled }),
        setDimOverlays: (enabled) => commandTransport.dispatch({ kind: "set-dim-overlays", enabled }),
    },
    window: {
        platform,
        beginDrag: () => commandTransport.dispatch({ kind: "window-start-system-move" }),
        minimize: () => commandTransport.dispatch({ kind: "window-minimize" }),
        toggleMaximize: () => commandTransport.dispatch({ kind: "window-toggle-maximize" }),
        close: () => commandTransport.dispatch({ kind: "window-close" }),
    },
    browser: {
        get state() {
            return snapshot;
        },
        onStateChange: (listener) => {
            listeners.add(listener);
            return () => listeners.delete(listener);
        },
    },
};

window.photon = photon;

window.addEventListener("photon-state", (event: Event) => {
    const next = (event as CustomEvent<unknown>).detail;
    if (!isBrowserSnapshot(next)) return;
    snapshot = next;
    applyTheme(next.themeMode);
    for (const listener of listeners) listener(next);
});

function isBrowserSnapshot(value: unknown): value is BrowserSnapshot {
    if (!value || typeof value !== "object") return false;
    const candidate = value as Partial<BrowserSnapshot>;
    return (
        Array.isArray(candidate.tabs) &&
        typeof candidate.activeTabId === "string" &&
        parseThemeMode(candidate.themeMode) !== undefined &&
        typeof candidate.forceDarkPages === "boolean" &&
        typeof candidate.dimOverlays === "boolean"
    );
}

function applyTheme(mode: ThemeMode): void {
    document.documentElement.dataset.theme = mode;
}

applyTheme(snapshot.themeMode);

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Photon Web UI root element is missing");

createRoot(rootElement).render(<App api={photon} />);
