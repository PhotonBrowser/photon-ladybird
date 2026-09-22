import { useEffect, useState, type FormEvent } from "react";

import type { BrowserState, PhotonApi, PhotonNavigation } from "./types";

interface AppProps {
    api: PhotonApi;
}

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
        if (value) navigation.navigate(value);
    };

    return (
        <header className="toolbar" data-photon-input="capture">
            <button type="button" aria-label="Back" disabled={!state.canGoBack} onClick={navigation.back}>
                ←
            </button>
            <button type="button" aria-label="Forward" disabled={!state.canGoForward} onClick={navigation.forward}>
                →
            </button>
            <button type="button" aria-label="Reload" onClick={navigation.reload}>
                ↻
            </button>
            <form onSubmit={submitAddress}>
                <input
                    id="address"
                    aria-label="Address"
                    autoComplete="off"
                    value={address}
                    onChange={(event) => setAddress(event.target.value)}
                />
            </form>
            <button type="button" aria-label="Open overlay" onClick={onOpenPopover}>
                Overlay test
            </button>
            <span id="status" aria-live="polite">
                {state.loading ? "Loading…" : state.title || "Photon"}
            </span>
        </header>
    );
}

interface PopoverProps {
    open: boolean;
    onClose(): void;
}

function Popover({ open, onClose }: PopoverProps) {
    return (
        <section className={open ? "popover open" : "popover"} aria-hidden={!open} data-photon-input="capture">
            <strong>Photon popover</strong>
            <p>This is a separate privileged document above the webpage.</p>
            <button type="button" onClick={onClose}>
                Close
            </button>
        </section>
    );
}

export default function App({ api }: AppProps) {
    const [popoverOpen, setPopoverOpen] = useState(false);

    useEffect(() => {
        const closeOnEscape = (event: KeyboardEvent) => {
            if (event.key === "Escape") setPopoverOpen(false);
        };

        window.addEventListener("keydown", closeOnEscape);
        return () => window.removeEventListener("keydown", closeOnEscape);
    }, []);

    return (
        <>
            <Toolbar navigation={api.navigation} state={api.browser.state} onOpenPopover={() => setPopoverOpen(true)} />
            <Popover open={popoverOpen} onClose={() => setPopoverOpen(false)} />
        </>
    );
}
