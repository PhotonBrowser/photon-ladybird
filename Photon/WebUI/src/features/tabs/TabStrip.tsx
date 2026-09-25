// SPDX-License-Identifier: GPL-3.0-only
import { Globe, LoaderCircle, Minus, Plus, Square, X } from "lucide-react";
import { memo, useEffect, useRef, useState } from "react";

import photonLogoMonotone from "../../assets/photon-logo-monotone.svg";
import type { BrowserTab, PhotonApi } from "../../types";
import { IconButton } from "../../ui/IconButton";
import { AnimatePresence, Motion } from "../../ui/Motion";

interface TabStripProps {
    api: PhotonApi;
    tabs: BrowserTab[];
    activeTabId: string;
}

export function TabStrip({ api, tabs, activeTabId }: TabStripProps): React.JSX.Element {
    const draggedTabId = useRef<string | null>(null);
    const tabsRef = useRef(tabs);
    tabsRef.current = tabs;

    return (
        <div className="photon-tab-strip">
            <div aria-label="Open tabs" className="photon-tabs" role="tablist">
                <AnimatePresence initial={false}>
                    {tabs.map((tab) => (
                        <Motion key={tab.id} className="photon-motion-tab" layout preset="tab">
                            <TabEntry
                                active={tab.id === activeTabId}
                                api={api}
                                draggedTabId={draggedTabId}
                                tab={tab}
                                tabsRef={tabsRef}
                            />
                        </Motion>
                    ))}
                </AnimatePresence>
            </div>
            <IconButton
                ariaLabel="New tab"
                className="photon-new-tab"
                size="sm"
                variant="ghost"
                onPress={() => api.tabs.create()}
            >
                <Plus aria-hidden="true" />
            </IconButton>
            {api.window.platform !== "macos" && (
                <div aria-label="Window controls" className="photon-window-controls" role="toolbar">
                    <IconButton
                        ariaLabel="Minimize window"
                        className="photon-window-control"
                        size="sm"
                        variant="ghost"
                        onPress={api.window.minimize}
                    >
                        <Minus aria-hidden="true" className="photon-window-icon photon-window-minimize-icon" />
                    </IconButton>
                    <IconButton
                        ariaLabel="Maximize or restore window"
                        className="photon-window-control"
                        size="sm"
                        variant="ghost"
                        onPress={api.window.toggleMaximize}
                    >
                        <Square aria-hidden="true" className="photon-window-icon photon-window-maximize-icon" />
                    </IconButton>
                    <IconButton
                        ariaLabel="Close window"
                        className="photon-window-control photon-window-close"
                        size="sm"
                        variant="ghost"
                        onPress={api.window.close}
                    >
                        <X aria-hidden="true" className="photon-window-icon photon-window-close-icon" />
                    </IconButton>
                </div>
            )}
        </div>
    );
}

const TabEntry = memo(function TabEntry({
    active,
    api,
    draggedTabId,
    tab,
    tabsRef,
}: {
    active: boolean;
    api: PhotonApi;
    draggedTabId: { current: string | null };
    tab: BrowserTab;
    tabsRef: { current: BrowserTab[] };
}): React.JSX.Element {
    const reorder = (): void => {
        const draggedId = draggedTabId.current;
        draggedTabId.current = null;
        if (!draggedId || draggedId === tab.id) return;

        const next = [...tabsRef.current];
        const fromIndex = next.findIndex((candidate) => candidate.id === draggedId);
        const toIndex = next.findIndex((candidate) => candidate.id === tab.id);
        if (fromIndex < 0 || toIndex < 0) return;
        const [moved] = next.splice(fromIndex, 1);
        if (!moved) return;
        next.splice(toIndex, 0, moved);
        api.tabs.reorder(next.map((candidate) => candidate.id));
    };

    return (
        <TabTooltip tab={tab}>
            <div className="photon-tab-entry" data-active={active} data-tab-id={tab.id} role="presentation">
                <button
                    aria-selected={active}
                    aria-busy={tab.loading}
                    className={active ? "photon-tab photon-tab-active" : "photon-tab"}
                    role="tab"
                    tabIndex={active ? 0 : -1}
                    draggable
                    type="button"
                    onClick={() => api.tabs.select(tab.id)}
                    onKeyDown={(event) => focusAdjacentTab(event, api)}
                    onDragStart={(event) => {
                        draggedTabId.current = tab.id;
                        event.dataTransfer.effectAllowed = "move";
                    }}
                    onDragOver={(event) => event.preventDefault()}
                    onDrop={(event) => {
                        event.preventDefault();
                        reorder();
                    }}
                    onDragEnd={() => {
                        draggedTabId.current = null;
                    }}
                >
                    <span className="photon-tab-icon" aria-hidden="true">
                        {tab.internalPage ? (
                            <img alt="" className="photon-tab-logo" src={photonLogoMonotone} />
                        ) : (
                            <WebTabIcon
                                loading={tab.loading}
                                faviconUrl={tab.faviconUrl}
                                key={`${tab.id}:${tab.faviconUrl ?? ""}`}
                            />
                        )}
                    </span>
                    <span className="photon-tab-title">{tab.title || "New Tab"}</span>
                </button>
                <IconButton
                    ariaLabel={`Close ${tab.title || "tab"}`}
                    className="photon-tab-close"
                    size="sm"
                    variant="ghost"
                    onPress={() => api.tabs.close(tab.id)}
                >
                    <X aria-hidden="true" />
                </IconButton>
            </div>
        </TabTooltip>
    );
});

function TabTooltip({ tab, children }: { tab: BrowserTab; children: React.ReactNode }): React.JSX.Element {
    const [open, setOpen] = useState(false);
    const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

    useEffect(
        () => () => {
            if (timer.current) clearTimeout(timer.current);
        },
        [],
    );

    const startDelay = (): void => {
        if (timer.current) clearTimeout(timer.current);
        timer.current = setTimeout(() => setOpen(true), 1000);
    };
    const cancelDelay = (): void => {
        if (timer.current) clearTimeout(timer.current);
        timer.current = null;
        setOpen(false);
    };

    return (
        <div className="photon-tab-tooltip-trigger" onPointerEnter={startDelay} onPointerLeave={cancelDelay}>
            {children}
            {open ? (
                <div aria-hidden="true" className="photon-tab-tooltip">
                    <span className="photon-tab-hover-title">{tab.title || "New Tab"}</span>
                    <span className="photon-tab-hover-site">{tabSite(tab.url, tab.internalPage)}</span>
                </div>
            ) : null}
        </div>
    );
}

function tabSite(url: string, internalPage: BrowserTab["internalPage"]): string {
    if (internalPage) return "Photon";
    try {
        return new URL(url).host || url;
    } catch {
        return url;
    }
}

function focusAdjacentTab(event: React.KeyboardEvent<HTMLButtonElement>, api: PhotonApi): void {
    const tabs = Array.from(
        event.currentTarget.closest(".photon-tabs")?.querySelectorAll<HTMLButtonElement>("[role='tab']") ?? [],
    );
    const currentIndex = tabs.indexOf(event.currentTarget);
    let nextIndex: number | undefined;

    if (event.key === "ArrowRight") nextIndex = (currentIndex + 1) % tabs.length;
    else if (event.key === "ArrowLeft") nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = tabs.length - 1;
    else return;

    event.preventDefault();
    const nextTab = tabs[nextIndex];
    nextTab?.focus();
    const tabId = nextTab?.closest<HTMLElement>("[data-tab-id]")?.dataset.tabId;
    if (tabId) api.tabs.select(tabId);
}

const WebTabIcon = memo(function WebTabIcon({
    faviconUrl,
    loading,
}: {
    faviconUrl: string | null;
    loading: boolean;
}): React.JSX.Element {
    const [failed, setFailed] = useState(false);
    if (loading) return <LoaderCircle aria-hidden="true" className="photon-spinner" />;
    if (!faviconUrl || failed) return <Globe aria-hidden="true" />;

    return (
        <img
            alt=""
            className="photon-tab-favicon"
            height={14}
            src={faviconUrl}
            width={14}
            onError={() => setFailed(true)}
        />
    );
});
