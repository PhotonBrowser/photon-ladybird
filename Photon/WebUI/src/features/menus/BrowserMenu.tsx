import { Dropdown } from "@heroui/react";
import { MoreVertical, Plus, Settings2 } from "lucide-react";
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
            <Dropdown.Trigger aria-label="Browser menu" className="photon-icon-button photon-toolbar-button">
                <MoreVertical aria-hidden="true" size={18} />
            </Dropdown.Trigger>
            <Dropdown.Popover className="photon-popover photon-menu-popover" placement="bottom end">
                <Dropdown.Menu aria-label="Browser menu" autoFocus={false}>
                    <Dropdown.Item
                        id="new-tab"
                        className="photon-menu-item"
                        onAction={() => runCommand(() => api.tabs.create())}
                    >
                        <Plus aria-hidden="true" size={17} />
                        <span>New tab</span>
                        <kbd>Ctrl+T</kbd>
                    </Dropdown.Item>
                    <Dropdown.Item
                        id="settings"
                        className="photon-menu-item"
                        onAction={() => runCommand(() => api.tabs.openSettings())}
                    >
                        <Settings2 aria-hidden="true" size={17} />
                        <span>Settings</span>
                    </Dropdown.Item>
                </Dropdown.Menu>
            </Dropdown.Popover>
        </Dropdown>
    );
}
