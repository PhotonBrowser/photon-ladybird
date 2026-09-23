import { ArrowLeft, ArrowRight, RotateCw } from "lucide-react";

import type { BrowserSnapshot, PhotonApi } from "../../types";
import { IconButton } from "../../ui/IconButton";

interface NavigationControlsProps {
    api: PhotonApi;
    snapshot: BrowserSnapshot;
}

export function NavigationControls({ api, snapshot }: NavigationControlsProps): React.JSX.Element {
    const tab = snapshot.tabs.find((candidate) => candidate.id === snapshot.activeTabId);
    return (
        <nav aria-label="Page navigation" className="photon-navigation-controls">
            <IconButton
                ariaLabel="Back"
                className="photon-toolbar-button"
                isDisabled={!tab?.canGoBack}
                size="sm"
                variant="ghost"
                onPress={() => api.navigation.back()}
            >
                <ArrowLeft aria-hidden="true" size={18} />
            </IconButton>
            <IconButton
                ariaLabel="Forward"
                className="photon-toolbar-button"
                isDisabled={!tab?.canGoForward}
                size="sm"
                variant="ghost"
                onPress={() => api.navigation.forward()}
            >
                <ArrowRight aria-hidden="true" size={18} />
            </IconButton>
            <IconButton
                ariaLabel={tab?.loading ? "Reloading page" : "Reload"}
                className="photon-toolbar-button"
                size="sm"
                variant="ghost"
                onPress={() => api.navigation.reload()}
            >
                <RotateCw aria-hidden="true" className={tab?.loading ? "photon-reload-loading" : undefined} size={17} />
            </IconButton>
        </nav>
    );
}
