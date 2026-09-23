// SPDX-License-Identifier: GPL-3.0-only
import { Dropdown } from "@heroui/react";
import { Plus, Settings2 } from "lucide-react";
import { EllipsisVertical, X } from "lucide";
import { MorphIcon } from "morphicons/react";
import { useRef } from "react";

import type { PhotonApi } from "../../types";

interface BrowserMenuProps {
    api: PhotonApi;
    isOpen: boolean;
    onOpenChange(open: boolean, notifyNative?: boolean): void;
}

export function BrowserMenu({ api, isOpen, onOpenChange }: BrowserMenuProps): React.JSX.Element {
    const suppressCloseCommand = useRef(false);
    const handleOpenChange = (nextOpen: boolean): void => {
        const notifyNative = nextOpen || !suppressCloseCommand.current;
        if (!nextOpen) suppressCloseCommand.current = false;
        onOpenChange(nextOpen, notifyNative);
    };
    const runCommand = (command: () => void): void => {
        suppressCloseCommand.current = true;
        onOpenChange(false, false);
        command();
        queueMicrotask(() => {
            suppressCloseCommand.current = false;
        });
    };

    return (
        <Dropdown isOpen={isOpen} onOpenChange={handleOpenChange}>
            <Dropdown.Trigger
                aria-expanded={isOpen}
                aria-label="Browser menu"
                className="photon-icon-button photon-toolbar-button"
            >
                <MorphIcon
                    aria-hidden="true"
                    className="photon-icon photon-menu-toggle-icon"
                    icon={isOpen ? X : EllipsisVertical}
                    reducedMotion="user"
                    spring="snappy"
                />
            </Dropdown.Trigger>
            <Dropdown.Popover className="photon-popover photon-menu-popover" placement="bottom end">
                <Dropdown.Menu aria-label="Browser menu">
                    <Dropdown.Item
                        id="new-tab"
                        aria-keyshortcuts="Control+T"
                        className="photon-menu-item"
                        onAction={() => runCommand(() => api.tabs.create())}
                    >
                        <Plus aria-hidden="true" />
                        <span>New tab</span>
                        <kbd>Ctrl+T</kbd>
                    </Dropdown.Item>
                    <Dropdown.Item
                        id="settings"
                        className="photon-menu-item"
                        onAction={() => runCommand(() => api.tabs.openSettings())}
                    >
                        <Settings2 aria-hidden="true" />
                        <span>Settings</span>
                    </Dropdown.Item>
                </Dropdown.Menu>
            </Dropdown.Popover>
        </Dropdown>
    );
}
