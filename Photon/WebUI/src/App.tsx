import { useEffect, useRef, useState } from "react";

import { Toolbar } from "./browser/Toolbar";
import { TabStrip } from "./features/tabs/TabStrip";
import { NewTabPage } from "./pages/NewTabPage";
import { SettingsPage } from "./pages/SettingsPage";
import { useBrowserSnapshot } from "./features/browser/useBrowserSnapshot";
import type { BrowserOverlay, PhotonApi } from "./types";

interface AppProps {
    api: PhotonApi;
}

export default function App({ api }: AppProps): React.JSX.Element {
    const snapshot = useBrowserSnapshot(api);
    const addressInput = useRef<HTMLInputElement>(null);
    const [activeOverlay, setActiveOverlay] = useState<BrowserOverlay | null>(null);
    const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId);
    const activeTabKey = `${activeTab?.id ?? ""}:${activeTab?.url ?? ""}`;
    const previousActiveTabKey = useRef(activeTabKey);

    const setOverlayOpen = (name: BrowserOverlay, open: boolean, notifyNative = true): void => {
        if (notifyNative) api.ui.setCaptureRegion(name, open);
        setActiveOverlay((current) => {
            if (open) return name;
            return current === name ? null : current;
        });
    };

    useEffect(() => {
        if (previousActiveTabKey.current === activeTabKey) return;
        previousActiveTabKey.current = activeTabKey;
        if (activeOverlay) api.ui.setCaptureRegion(activeOverlay, false);
        setActiveOverlay(null);
    }, [activeOverlay, activeTabKey, api]);

    useEffect(() => {
        const focusAddressBar = (): void => {
            if (activeOverlay) {
                api.ui.setCaptureRegion(activeOverlay, false);
                setActiveOverlay(null);
            }
            addressInput.current?.focus();
            addressInput.current?.select();
        };
        window.addEventListener("photon-focus-address", focusAddressBar);
        return () => window.removeEventListener("photon-focus-address", focusAddressBar);
    }, [activeOverlay, api]);

    return (
        <div className="photon-shell">
            <div className="photon-chrome">
                <div className="photon-titlebar">
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
            {activeOverlay && (
                <button
                    aria-label="Close overlay"
                    className="photon-overlay-scrim"
                    onClick={() => setOverlayOpen(activeOverlay, false)}
                    tabIndex={-1}
                    type="button"
                />
            )}
            {activeTab?.internalPage === "new-tab" ? <NewTabPage /> : null}
            {activeTab?.internalPage === "settings" ? <SettingsPage api={api} snapshot={snapshot} /> : null}
        </div>
    );
}
