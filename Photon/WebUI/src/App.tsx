// SPDX-License-Identifier: GPL-3.0-only
import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { Toolbar } from "./browser/Toolbar";
import { TabStrip } from "./features/tabs/TabStrip";
import { NewTabPage } from "./pages/NewTabPage";
import { SettingsPage } from "./pages/SettingsPage";
import { useBrowserSnapshot } from "./features/browser/useBrowserSnapshot";
import type { BrowserOverlay, PhotonApi } from "./types";

type PageTooltip = { text: string; x: number; y: number };

function isPageTooltip(value: unknown): value is PageTooltip {
    if (typeof value !== "object" || value === null) return false;
    const tooltip = value as Partial<PageTooltip>;
    return typeof tooltip.text === "string" && typeof tooltip.x === "number" && typeof tooltip.y === "number";
}

function BrowserTooltip({ tooltip }: { tooltip: PageTooltip }): React.JSX.Element {
    const tooltipRef = useRef<HTMLDivElement>(null);
    const [position, setPosition] = useState({ left: tooltip.x + 8, top: tooltip.y + 18 });

    useLayoutEffect(() => {
        const bounds = tooltipRef.current?.getBoundingClientRect();
        if (!bounds) return;
        const left = Math.max(8, Math.min(tooltip.x + 8, window.innerWidth - bounds.width - 8));
        const below = tooltip.y + 18 + bounds.height <= window.innerHeight - 8;
        const top = below ? tooltip.y + 18 : Math.max(8, tooltip.y - bounds.height - 8);
        setPosition({ left, top });
    }, [tooltip]);

    return (
        <div
            aria-hidden="true"
            className="photon-page-tooltip"
            ref={tooltipRef}
            style={{ left: position.left, top: position.top }}
        >
            {tooltip.text}
        </div>
    );
}

interface AppProps {
    api: PhotonApi;
}

export default function App({ api }: AppProps): React.JSX.Element {
    const snapshot = useBrowserSnapshot(api);
    const addressInput = useRef<HTMLInputElement>(null);
    const [activeOverlay, setActiveOverlay] = useState<BrowserOverlay | null>(null);
    const [pageTooltip, setPageTooltip] = useState<PageTooltip | null>(null);
    const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId);
    const activeTabKey = `${activeTab?.id ?? ""}:${activeTab?.url ?? ""}`;
    const previousActiveTabKey = useRef(activeTabKey);

    const setOverlayOpen = (name: BrowserOverlay, open: boolean, notifyNative = true): void => {
        if (notifyNative && name !== "settings-theme") api.ui.setCaptureRegion(name, open);
        setActiveOverlay((current) => {
            if (open) return name;
            return current === name ? null : current;
        });
    };

    useEffect(() => {
        if (previousActiveTabKey.current === activeTabKey) return;
        previousActiveTabKey.current = activeTabKey;
        setPageTooltip(null);
        if (activeOverlay && activeOverlay !== "settings-theme") api.ui.setCaptureRegion(activeOverlay, false);
        setActiveOverlay(null);
    }, [activeOverlay, activeTabKey, api]);

    useLayoutEffect(() => {
        if (activeTab?.internalPage !== "new-tab") return;
        addressInput.current?.focus();
    }, [activeTab?.id, activeTab?.internalPage]);

    useEffect(() => {
        let showTimer = 0;
        const clearTooltip = (): void => {
            window.clearTimeout(showTimer);
            setPageTooltip(null);
        };
        const showTooltip = (event: Event): void => {
            const detail = (event as CustomEvent<unknown>).detail;
            if (!isPageTooltip(detail)) return;
            window.clearTimeout(showTimer);
            setPageTooltip(null);
            showTimer = window.setTimeout(() => setPageTooltip(detail), 600);
        };
        window.addEventListener("photon-ui-page-tooltip", showTooltip);
        window.addEventListener("photon-ui-page-tooltip-clear", clearTooltip);
        return () => {
            window.clearTimeout(showTimer);
            window.removeEventListener("photon-ui-page-tooltip", showTooltip);
            window.removeEventListener("photon-ui-page-tooltip-clear", clearTooltip);
        };
    }, []);

    useEffect(() => {
        if (activeOverlay) setPageTooltip(null);
    }, [activeOverlay]);

    useEffect(() => {
        const focusAddressBar = (): void => {
            if (activeOverlay) {
                if (activeOverlay !== "settings-theme") api.ui.setCaptureRegion(activeOverlay, false);
                setActiveOverlay(null);
            }
            addressInput.current?.focus();
            addressInput.current?.select();
        };
        window.addEventListener("photon-ui-focus-address", focusAddressBar);
        return () => window.removeEventListener("photon-ui-focus-address", focusAddressBar);
    }, [activeOverlay, api]);

    return (
        <div className="photon-shell" data-dim-overlays={snapshot.dimOverlays}>
            <div className="photon-chrome">
                <div
                    className="photon-titlebar"
                    onPointerDown={(event) => {
                        if (event.button !== 0 || (event.target as Element).closest("button, input, a, [role='tab']"))
                            return;
                        // Handle the second press before another native move can claim the gesture.
                        if (event.detail === 2) {
                            api.window.maximize();
                            return;
                        }
                        if (event.detail > 2) return;
                        api.window.beginDrag();
                    }}
                >
                    <TabStrip activeTabId={snapshot.activeTabId} api={api} tabs={snapshot.tabs} />
                </div>
                <Toolbar
                    addressInput={addressInput}
                    api={api}
                    menuOpen={activeOverlay === "browser-menu"}
                    onMenuOpenChange={(open, notifyNative) => setOverlayOpen("browser-menu", open, notifyNative)}
                    onSiteInfoOpenChange={(open, notifyNative) => setOverlayOpen("site-info", open, notifyNative)}
                    siteInfoOpen={activeOverlay === "site-info"}
                    snapshot={snapshot}
                />
            </div>
            {activeTab && !activeTab.internalPage ? <div aria-hidden="true" className="photon-page-frame" /> : null}
            {activeOverlay && (
                <button
                    aria-label="Close overlay"
                    className="photon-overlay-scrim"
                    onClick={() => setOverlayOpen(activeOverlay, false)}
                    tabIndex={-1}
                    type="button"
                />
            )}
            {pageTooltip && !activeOverlay ? <BrowserTooltip tooltip={pageTooltip} /> : null}
            {activeTab?.internalPage === "new-tab" ? <NewTabPage /> : null}
            {activeTab?.internalPage === "settings" ? (
                <SettingsPage
                    api={api}
                    snapshot={snapshot}
                    themeDropdownOpen={activeOverlay === "settings-theme"}
                    onThemeDropdownOpenChange={(open) => setOverlayOpen("settings-theme", open, false)}
                />
            ) : null}
        </div>
    );
}
