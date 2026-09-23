import { createRoot } from "react-dom/client";

import App from "./App";
import "./styles.css";
import type { BrowserSnapshot, PhotonApi, ThemeMode } from "./types";

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
};

const snapshotFromDevServer = (): BrowserSnapshot | undefined => {
    const value = new URLSearchParams(window.location.search).get("photonInitialState");
    if (!value) return undefined;
    try {
        return JSON.parse(value) as BrowserSnapshot;
    } catch {
        return undefined;
    }
};

let snapshot = window.__photonInitialState ?? snapshotFromDevServer() ?? developmentSnapshot;
const listeners = new Set<(state: BrowserSnapshot) => void>();
const nativeBridgeAvailable =
    window.__photonInitialState !== undefined || new URLSearchParams(window.location.search).has("photonInitialState");

const emitCommand = (name: string, value?: string): void => {
    if (!nativeBridgeAvailable) return;
    const query = value === undefined ? "" : `?value=${encodeURIComponent(value)}`;
    window.location.href = `photon-command://${name}${query}`;
};

const parseThemeMode = (value: unknown): ThemeMode | undefined =>
    value === "system" || value === "light" || value === "dark" ? value : undefined;

const photon: PhotonApi = {
    navigation: {
        navigate: (url) => emitCommand("navigate", url),
        back: () => emitCommand("back"),
        forward: () => emitCommand("forward"),
        reload: () => emitCommand("reload"),
    },
    tabs: {
        create: () => emitCommand("new-tab"),
        openSettings: () => emitCommand("open-settings"),
        select: (tabId) => emitCommand("select-tab", tabId),
        close: (tabId) => emitCommand("close-tab", tabId),
        reorder: (tabIds) => emitCommand("reorder-tabs", tabIds.join(",")),
    },
    ui: {
        setCaptureRegion: (name, open) => emitCommand("capture", `${name}:${open ? "open" : "close"}`),
    },
    settings: {
        setTheme: (mode) => emitCommand("set-theme", mode),
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
        parseThemeMode(candidate.themeMode) !== undefined
    );
}

function applyTheme(mode: ThemeMode): void {
    document.documentElement.dataset.theme = mode;
}

applyTheme(snapshot.themeMode);

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Photon Web UI root element is missing");

createRoot(rootElement).render(<App api={photon} />);
