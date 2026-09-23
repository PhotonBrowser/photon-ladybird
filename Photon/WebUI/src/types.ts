export type InternalPageId = "new-tab" | "settings";
export type ThemeMode = "system" | "light" | "dark";
export type BrowserOverlay = "site-info" | "browser-menu";

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
    setCaptureRegion(name: BrowserOverlay, open: boolean): void;
}

export interface PhotonSettings {
    setTheme(mode: ThemeMode): void;
}

export interface PhotonApi {
    navigation: PhotonNavigation;
    tabs: PhotonTabs;
    ui: PhotonUi;
    settings: PhotonSettings;
    browser: PhotonBrowser;
}
