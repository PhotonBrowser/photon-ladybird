// SPDX-License-Identifier: GPL-3.0-only
import { Tooltip } from "@heroui/react";
import { ArrowLeft, ArrowRight, RotateCw, X } from "lucide-react";

import type { BrowserSnapshot, PhotonApi } from "../../types";
import { IconButton } from "../../ui/IconButton";

interface NavigationControlsProps {
    api: PhotonApi;
    snapshot: BrowserSnapshot;
}

export function NavigationControls({ api, snapshot }: NavigationControlsProps): React.JSX.Element {
    const tab = snapshot.tabs.find((candidate) => candidate.id === snapshot.activeTabId);
    return (
        <nav aria-busy={tab?.loading ?? false} aria-label="Page navigation" className="photon-navigation-controls">
            <Tooltip>
                <Tooltip.Trigger>
                    <IconButton
                        ariaLabel="Back"
                        className="photon-toolbar-button"
                        isDisabled={!tab?.canGoBack}
                        size="sm"
                        variant="ghost"
                        onPress={() => api.navigation.back()}
                    >
                        <ArrowLeft aria-hidden="true" />
                    </IconButton>
                </Tooltip.Trigger>
                <Tooltip.Content className="photon-control-tooltip">Back</Tooltip.Content>
            </Tooltip>
            <Tooltip>
                <Tooltip.Trigger>
                    <IconButton
                        ariaLabel="Forward"
                        className="photon-toolbar-button"
                        isDisabled={!tab?.canGoForward}
                        size="sm"
                        variant="ghost"
                        onPress={() => api.navigation.forward()}
                    >
                        <ArrowRight aria-hidden="true" />
                    </IconButton>
                </Tooltip.Trigger>
                <Tooltip.Content className="photon-control-tooltip">Forward</Tooltip.Content>
            </Tooltip>
            <Tooltip>
                <Tooltip.Trigger>
                    <IconButton
                        ariaLabel={tab?.loading ? "Stop loading" : "Reload page"}
                        className="photon-toolbar-button"
                        size="sm"
                        variant="ghost"
                        onPress={() => api.navigation.reload()}
                    >
                        {tab?.loading ? <X aria-hidden="true" /> : <RotateCw aria-hidden="true" />}
                    </IconButton>
                </Tooltip.Trigger>
                <Tooltip.Content className="photon-control-tooltip">
                    {tab?.loading ? "Stop loading" : "Reload"}
                </Tooltip.Content>
            </Tooltip>
        </nav>
    );
}
