// SPDX-License-Identifier: GPL-3.0-only
import { useEffect, useState } from "react";

import type { BrowserSnapshot, PhotonApi } from "../../types";

export function useBrowserSnapshot(api: PhotonApi): BrowserSnapshot {
    const [snapshot, setSnapshot] = useState(api.browser.state);
    useEffect(
        () =>
            api.browser.onStateChange((next) => {
                setSnapshot((current) => {
                    const tabs = next.tabs.map((tab, index) => {
                        const previous = current.tabs[index];
                        return previous && sameTab(previous, tab) ? previous : tab;
                    });
                    const unchanged =
                        current.activeTabId === next.activeTabId &&
                        current.themeMode === next.themeMode &&
                        current.forceDarkPages === next.forceDarkPages &&
                        current.dimOverlays === next.dimOverlays &&
                        current.windowTintOpacity === next.windowTintOpacity &&
                        current.tabs.length === tabs.length &&
                        tabs.every((tab, index) => tab === current.tabs[index]);
                    return unchanged ? current : { ...next, tabs };
                });
            }),
        [api],
    );
    return snapshot;
}

function sameTab(left: BrowserSnapshot["tabs"][number], right: BrowserSnapshot["tabs"][number]): boolean {
    return (
        left.id === right.id &&
        left.url === right.url &&
        left.title === right.title &&
        left.faviconUrl === right.faviconUrl &&
        left.internalPage === right.internalPage &&
        left.loading === right.loading &&
        left.canGoBack === right.canGoBack &&
        left.canGoForward === right.canGoForward
    );
}
