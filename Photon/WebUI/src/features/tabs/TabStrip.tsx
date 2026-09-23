// SPDX-License-Identifier: GPL-3.0-only
import { Minus, Plus, Square, X } from "lucide-react";
import { Globe, LoaderCircle } from "lucide";
import { MorphIcon } from "morphicons/react";
import { useRef, useState } from "react";

import photonLogoMonotone from "../../assets/photon-logo-monotone.svg";
import type { BrowserTab, PhotonApi } from "../../types";
import { IconButton } from "../../ui/IconButton";

interface TabStripProps {
    api: PhotonApi;
    tabs: BrowserTab[];
    activeTabId: string;
}

export function TabStrip({ api, tabs, activeTabId }: TabStripProps): React.JSX.Element {
    const draggedTabId = useRef<string | null>(null);

    const reorder = (targetId: string): void => {
        const draggedId = draggedTabId.current;
        draggedTabId.current = null;
        if (!draggedId || draggedId === targetId) return;

        const next = [...tabs];
        const fromIndex = next.findIndex((tab) => tab.id === draggedId);
        const toIndex = next.findIndex((tab) => tab.id === targetId);
        if (fromIndex < 0 || toIndex < 0) return;
        const [moved] = next.splice(fromIndex, 1);
        if (!moved) return;
        next.splice(toIndex, 0, moved);
        api.tabs.reorder(next.map((tab) => tab.id));
    };

    return (
        <div className="photon-tab-strip">
            <div aria-label="Open tabs" className="photon-tabs" role="tablist">
                {tabs.map((tab) => {
                    const active = tab.id === activeTabId;
                    return (
                        <div
                            key={tab.id}
                            className="photon-tab-entry"
                            data-active={active}
                            data-tab-id={tab.id}
                            role="presentation"
                        >
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
                                    reorder(tab.id);
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
                    );
                })}
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
                        <Minus aria-hidden="true" />
                    </IconButton>
                    <IconButton
                        ariaLabel="Maximize or restore window"
                        className="photon-window-control"
                        size="sm"
                        variant="ghost"
                        onPress={api.window.toggleMaximize}
                    >
                        <Square aria-hidden="true" />
                    </IconButton>
                    <IconButton
                        ariaLabel="Close window"
                        className="photon-window-control photon-window-close"
                        size="sm"
                        variant="ghost"
                        onPress={api.window.close}
                    >
                        <X aria-hidden="true" />
                    </IconButton>
                </div>
            )}
        </div>
    );
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

function WebTabIcon({ faviconUrl, loading }: { faviconUrl: string | null; loading: boolean }): React.JSX.Element {
    const [failed, setFailed] = useState(false);
    const fallback = !faviconUrl || failed;
    return (
        <>
            <MorphIcon
                aria-hidden="true"
                className="photon-icon photon-tab-morph-icon"
                icon={loading ? LoaderCircle : fallback ? Globe : undefined}
                reducedMotion="user"
                spring="snappy"
            />
            {!fallback && (
                <img
                    alt=""
                    className="photon-tab-favicon"
                    height={14}
                    src={faviconUrl}
                    width={14}
                    onError={() => setFailed(true)}
                />
            )}
        </>
    );
}
