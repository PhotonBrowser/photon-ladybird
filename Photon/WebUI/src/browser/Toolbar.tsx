// SPDX-License-Identifier: GPL-3.0-only
import type { RefObject } from "react";

import type { BrowserSnapshot, PhotonApi } from "../types";
import { BrowserMenu } from "../features/menus/BrowserMenu";
import { NavigationControls } from "../features/navigation/NavigationControls";
import { Omnibox } from "../features/omnibox/Omnibox";

interface ToolbarProps {
    api: PhotonApi;
    snapshot: BrowserSnapshot;
    addressInput: RefObject<HTMLInputElement | null>;
    menuOpen: boolean;
    siteInfoOpen: boolean;
    onMenuOpenChange(open: boolean, notifyNative?: boolean): void;
    onSiteInfoOpenChange(open: boolean, notifyNative?: boolean): void;
}

export function Toolbar({
    api,
    snapshot,
    addressInput,
    menuOpen,
    siteInfoOpen,
    onMenuOpenChange,
    onSiteInfoOpenChange,
}: ToolbarProps): React.JSX.Element {
    return (
        <div aria-label="Browser controls" className="photon-toolbar" role="toolbar">
            <NavigationControls api={api} snapshot={snapshot} />
            <Omnibox
                api={api}
                inputRef={addressInput}
                onSiteInfoOpenChange={onSiteInfoOpenChange}
                siteInfoOpen={siteInfoOpen}
                snapshot={snapshot}
            />
            <BrowserMenu api={api} isOpen={menuOpen} onOpenChange={onMenuOpenChange} />
        </div>
    );
}
