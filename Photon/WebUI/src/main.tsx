import { useEffect, useState, type FormEvent } from "react";
import { createRoot } from "react-dom/client";

import type { BrowserState, PhotonApi, PhotonNavigation } from "./types";

const initialState: BrowserState = window.__photonInitialState ?? {
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

interface ToolbarProps {
    navigation: PhotonNavigation;
    state: BrowserState;
    onOpenPopover(): void;
}

function Toolbar({ navigation, state, onOpenPopover }: ToolbarProps) {
    const [address, setAddress] = useState(state.url);

    useEffect(() => {
        setAddress(state.url);
    }, [state.url]);

    const submitAddress = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const value = address.trim();
        if (value)
            navigation.navigate(value);
    };

    return (
        <header className="toolbar" data-photon-input="capture">
            <button type="button" aria-label="Back" disabled={!state.canGoBack} onClick={navigation.back}>←</button>
            <button type="button" aria-label="Forward" disabled={!state.canGoForward} onClick={navigation.forward}>→</button>
            <button type="button" aria-label="Reload" onClick={navigation.reload}>↻</button>
            <form onSubmit={submitAddress}>
                <input
                    id="address"
                    aria-label="Address"
                    autoComplete="off"
                    value={address}
                    onChange={(event) => setAddress(event.target.value)}
                />
            </form>
            <button type="button" aria-label="Open overlay" onClick={onOpenPopover}>Overlay test</button>
            <span id="status" aria-live="polite">{state.loading ? "Loading…" : state.title || "Photon"}</span>
        </header>
    );
}

interface PopoverProps {
    open: boolean;
    onClose(): void;
}

function Popover({ open, onClose }: PopoverProps) {
    return (
        <section
            className={open ? "popover open" : "popover"}
            aria-hidden={!open}
            data-photon-input="capture"
        >
            <strong>Photon popover</strong>
            <p>This is a separate privileged document above the webpage.</p>
            <button type="button" onClick={onClose}>Close</button>
        </section>
    );
}

interface PhotonChromeProps {
    api: PhotonApi;
}

function PhotonChrome({ api }: PhotonChromeProps) {
    const [popoverOpen, setPopoverOpen] = useState(false);

    useEffect(() => {
        const closeOnEscape = (event: KeyboardEvent) => {
            if (event.key === "Escape")
                setPopoverOpen(false);
        };

        window.addEventListener("keydown", closeOnEscape);
        return () => window.removeEventListener("keydown", closeOnEscape);
    }, []);

    return (
        <>
            <Toolbar
                navigation={api.navigation}
                state={api.browser.state}
                onOpenPopover={() => setPopoverOpen(true)}
            />
            <Popover open={popoverOpen} onClose={() => setPopoverOpen(false)} />
        </>
    );
}

window.photon = photon;

const rootElement = document.getElementById("root");
if (!rootElement)
    throw new Error("Photon Web UI root element is missing");

createRoot(rootElement).render(<PhotonChrome api={photon} />);
