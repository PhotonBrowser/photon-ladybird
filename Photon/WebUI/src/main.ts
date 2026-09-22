interface BrowserState {
    url: string;
    title: string;
    loading: boolean;
    canGoBack: boolean;
    canGoForward: boolean;
}

interface PhotonNavigation {
    navigate(url: string): void;
    back(): void;
    forward(): void;
    reload(): void;
}

interface PhotonBrowser {
    state: BrowserState;
}

interface PhotonApi {
    navigation: PhotonNavigation;
    browser: PhotonBrowser;
}

const state: BrowserState = window.__photonInitialState ?? {
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
    browser: { state },
};

const renderState = (): void => {
    const url = document.querySelector<HTMLInputElement>("#address");
    const back = document.querySelector<HTMLButtonElement>("#back");
    const forward = document.querySelector<HTMLButtonElement>("#forward");
    const status = document.querySelector<HTMLElement>("#status");
    if (!url || !back || !forward || !status) return;
    url.value = state.url;
    back.disabled = !state.canGoBack;
    forward.disabled = !state.canGoForward;
    status.textContent = state.loading ? "Loading…" : state.title || "Photon";
};

const openPopover = (open: boolean): void => {
    const popover = document.querySelector<HTMLElement>("#popover");
    if (!popover) return;
    popover.classList.toggle("open", open);
    popover.setAttribute("aria-hidden", String(!open));
};

document.querySelector<HTMLButtonElement>("#back")?.addEventListener("click", () => photon.navigation.back());
document.querySelector<HTMLButtonElement>("#forward")?.addEventListener("click", () => photon.navigation.forward());
document.querySelector<HTMLButtonElement>("#reload")?.addEventListener("click", () => photon.navigation.reload());
document.querySelector<HTMLButtonElement>("#overlay")?.addEventListener("click", () => openPopover(true));
document.querySelector<HTMLButtonElement>("#close-popover")?.addEventListener("click", () => openPopover(false));
document.querySelector<HTMLFormElement>("#address-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    const value = document.querySelector<HTMLInputElement>("#address")?.value.trim();
    if (value) photon.navigation.navigate(value);
});

window.addEventListener("keydown", (event) => {
    if (event.key === "Escape") openPopover(false);
});

window.photon = photon;
renderState();
