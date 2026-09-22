export interface BrowserState {
    url: string;
    title: string;
    loading: boolean;
    canGoBack: boolean;
    canGoForward: boolean;
}

export interface PhotonNavigation {
    navigate(url: string): void;
    back(): void;
    forward(): void;
    reload(): void;
}

export interface PhotonBrowser {
    state: BrowserState;
}

export interface PhotonApi {
    navigation: PhotonNavigation;
    browser: PhotonBrowser;
}
