import { Popover } from "@heroui/react";
import { Asterisk, LockKeyhole, ShieldAlert, X } from "lucide-react";

import type { BrowserTab } from "../../types";
import { IconButton } from "../../ui/IconButton";

interface SiteSecurityProps {
    isOpen: boolean;
    tab: BrowserTab | undefined;
    onOpenChange(open: boolean, notifyNative?: boolean): void;
}

export function SiteSecurity({ isOpen, tab, onOpenChange }: SiteSecurityProps): React.JSX.Element {
    const parsedUrl = tab?.internalPage ? null : safeUrl(tab?.url);
    const host = parsedUrl?.host;
    const secure = parsedUrl?.protocol === "https:";
    return host ? (
        <Popover isOpen={isOpen} onOpenChange={onOpenChange}>
            <Popover.Trigger aria-label="Site information" className="photon-icon-button photon-site-info-button">
                <Asterisk aria-hidden="true" size={19} strokeWidth={2.4} />
            </Popover.Trigger>
            <Popover.Content className="photon-popover photon-site-popover" placement="bottom start">
                <Popover.Dialog aria-label={`Site information for ${host}`}>
                    <header className="photon-popover-heading">
                        <strong>{host}</strong>
                        <IconButton
                            ariaLabel="Close site information"
                            className="photon-popover-close"
                            size="sm"
                            variant="ghost"
                            onPress={() => onOpenChange(false)}
                        >
                            <X aria-hidden="true" size={16} />
                        </IconButton>
                    </header>
                    <div className="photon-security-row">
                        {secure ? (
                            <LockKeyhole aria-hidden="true" size={18} />
                        ) : (
                            <ShieldAlert aria-hidden="true" size={18} />
                        )}
                        <span>{secure ? "Page loaded over HTTPS" : "Page is not using HTTPS"}</span>
                    </div>
                    <p className="photon-popover-note">
                        Photon displays the address and scheme used by the current page.
                    </p>
                </Popover.Dialog>
            </Popover.Content>
        </Popover>
    ) : (
        <IconButton
            ariaLabel="Site information"
            className="photon-site-info-button"
            isDisabled
            size="sm"
            variant="ghost"
        >
            <Asterisk aria-hidden="true" size={19} strokeWidth={2.4} />
        </IconButton>
    );
}

function safeUrl(value: string | undefined): URL | undefined {
    if (!value) return undefined;
    try {
        return new URL(value);
    } catch {
        return undefined;
    }
}
