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
    const [crashNotice, setCrashNotice] = useState<string | null>(null);
    const titlebarDragRegion = useRef(false);
    const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId);
    const activeTabId = activeTab?.id;
    const activeTabInternalPage = activeTab?.internalPage;
    const activeTabKey = `${activeTab?.id ?? ""}:${activeTab?.url ?? ""}`;
    const previousActiveTabKey = useRef(activeTabKey);
    const tabFocus = useRef({
        activeTabId: null as string | null,
        tabCount: snapshot.tabs.length,
        focusedTabIds: new Set<string>(),
    });

    const setOverlayOpen = (name: BrowserOverlay, open: boolean, notifyNative = true): void => {
        if (notifyNative && name !== "settings-theme") api.ui.setCaptureRegion(name, open);
        setActiveOverlay((current) => {
            if (open) return name;
            return current === name ? null : current;
        });
    };

    const updateTitlebarDragRegion = (target: EventTarget | null): void => {
        const element = target instanceof Element ? target : null;
        const enabled = Boolean(element && !element.closest("button, input, a, [role='tab']"));
        if (titlebarDragRegion.current === enabled) return;
        titlebarDragRegion.current = enabled;
        api.window.setTitlebarDragRegion(enabled);
    };

    useEffect(() => {
        if (previousActiveTabKey.current === activeTabKey) return;
        previousActiveTabKey.current = activeTabKey;
        setPageTooltip(null);
        if (activeOverlay && activeOverlay !== "settings-theme") api.ui.setCaptureRegion(activeOverlay, false);
        setActiveOverlay(null);
    }, [activeOverlay, activeTabKey, api]);

    useLayoutEffect(() => {
        const focusState = tabFocus.current;
        const isNewTab = focusState.activeTabId === null || snapshot.tabs.length > focusState.tabCount;
        const changedActiveTab = focusState.activeTabId !== activeTabId;
        focusState.activeTabId = activeTabId ?? null;
        focusState.tabCount = snapshot.tabs.length;

        if (!activeTabId || !changedActiveTab) return;
        if (isNewTab && activeTabInternalPage === "new-tab") focusState.focusedTabIds.add(activeTabId);
        if (focusState.focusedTabIds.has(activeTabId)) {
            addressInput.current?.focus();
            return;
        }
        addressInput.current?.blur();
    }, [activeTabId, activeTabInternalPage, snapshot.tabs]);

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
        let hideTimer = 0;
        const showCrashNotice = (message: string) => (): void => {
            window.clearTimeout(hideTimer);
            setCrashNotice(message);
            hideTimer = window.setTimeout(() => setCrashNotice(null), 4000);
        };
        const showPageCrash = showCrashNotice("Page crashed. Reloading…");
        const showChromeCrash = showCrashNotice("Photon restarted after a browser UI crash.");
        window.addEventListener("photon-ui-page-crashed", showPageCrash);
        window.addEventListener("photon-ui-chrome-crashed", showChromeCrash);
        return () => {
            window.clearTimeout(hideTimer);
            window.removeEventListener("photon-ui-page-crashed", showPageCrash);
            window.removeEventListener("photon-ui-chrome-crashed", showChromeCrash);
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
        const blurAddressBar = (): void => {
            if (activeTab) tabFocus.current.focusedTabIds.delete(activeTab.id);
            addressInput.current?.blur();
        };
        window.addEventListener("photon-ui-focus-address", focusAddressBar);
        window.addEventListener("photon-ui-blur-address", blurAddressBar);
        return () => {
            window.removeEventListener("photon-ui-focus-address", focusAddressBar);
            window.removeEventListener("photon-ui-blur-address", blurAddressBar);
        };
    }, [activeOverlay, activeTab, api]);

    return (
        <div className="photon-shell" data-dim-overlays={snapshot.dimOverlays}>
            <div className="photon-chrome">
                <div
                    className="photon-titlebar"
                    role="toolbar"
                    aria-label="Window title bar"
                    onPointerMove={(event) => updateTitlebarDragRegion(event.target)}
                    onPointerLeave={() => updateTitlebarDragRegion(null)}
                    onPointerDown={(event) => {
                        updateTitlebarDragRegion(event.target);
                        if (event.button !== 0 || event.detail !== 2) return;
                        const target = event.target as Element;
                        const isTabLabel = Boolean(
                            target.closest("[role='tab']") && !target.closest(".photon-tab-close"),
                        );
                        if (isTabLabel || !target.closest("button, input, a")) api.window.toggleMaximize();
                    }}
                >
                    <TabStrip activeTabId={snapshot.activeTabId} api={api} tabs={snapshot.tabs} />
                </div>
                <Toolbar
                    addressInput={addressInput}
                    api={api}
                    menuOpen={activeOverlay === "browser-menu"}
                    onMenuOpenChange={(open, notifyNative) => setOverlayOpen("browser-menu", open, notifyNative)}
                    onAddressFocus={(tabId) => tabFocus.current.focusedTabIds.add(tabId)}
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
            {crashNotice ? (
                <div aria-live="polite" className="photon-crash-notice" role="status">
                    {crashNotice}
                </div>
            ) : null}
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
