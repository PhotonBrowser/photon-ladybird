// SPDX-License-Identifier: GPL-3.0-only
export type InternalPageId = "new-tab" | "settings";
export type ThemeMode = "system" | "light" | "dark";
export type BrowserOverlay = "site-info" | "browser-menu" | "settings-theme";

export interface BrowserTab {
    id: string;
    url: string;
    title: string;
    faviconUrl: string | null;
    internalPage: InternalPageId | null;
    loading: boolean;
    canGoBack: boolean;
    canGoForward: boolean;
}

export interface BrowserSnapshot {
    tabs: BrowserTab[];
    activeTabId: string;
    themeMode: ThemeMode;
    forceDarkPages: boolean;
    dimOverlays: boolean;
    windowTintOpacity: number;
}

export interface PhotonBrowser {
    state: BrowserSnapshot;
    onStateChange(listener: (state: BrowserSnapshot) => void): () => void;
}

export interface PhotonNavigation {
    navigate(url: string): void;
    back(): void;
    forward(): void;
    reload(): void;
}

export interface PhotonTabs {
    create(): void;
    openSettings(): void;
    select(tabId: string): void;
    close(tabId: string): void;
    reorder(tabIds: string[]): void;
}

export interface PhotonUi {
    setCaptureRegion(name: "site-info" | "browser-menu", open: boolean): void;
}

export interface PhotonSettings {
    setTheme(mode: ThemeMode): void;
    setForceDarkPages(enabled: boolean): void;
    setDimOverlays(enabled: boolean): void;
    setWindowTintOpacity(opacity: number): void;
}

export interface PhotonWindow {
    platform: "macos" | "other";
    setTitlebarDragRegion(enabled: boolean): void;
    minimize(): void;
    maximize(): void;
    toggleMaximize(): void;
    close(): void;
}

export interface PhotonApi {
    navigation: PhotonNavigation;
    tabs: PhotonTabs;
    ui: PhotonUi;
    settings: PhotonSettings;
    window: PhotonWindow;
    browser: PhotonBrowser;
}
