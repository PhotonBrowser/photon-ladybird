import { Input } from "@heroui/react";
import { useEffect, useState, type FormEvent, type KeyboardEvent, type RefObject } from "react";

import type { BrowserSnapshot, PhotonApi } from "../../types";
import { SiteSecurity } from "./SiteSecurity";

interface OmniboxProps {
    api: PhotonApi;
    snapshot: BrowserSnapshot;
    inputRef: RefObject<HTMLInputElement | null>;
    siteInfoOpen: boolean;
    onSiteInfoOpenChange(open: boolean, notifyNative?: boolean): void;
}

export function Omnibox({
    api,
    snapshot,
    inputRef,
    siteInfoOpen,
    onSiteInfoOpenChange,
}: OmniboxProps): React.JSX.Element {
    const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId);
    const isInternalPage = Boolean(activeTab?.internalPage);
    const [draftAddress, setDraftAddress] = useState(isInternalPage ? "" : (activeTab?.url ?? ""));
    const [editingTabId, setEditingTabId] = useState<string | null>(null);
    const isEditing = editingTabId === activeTab?.id;

    useEffect(() => {
        if (editingTabId !== activeTab?.id) {
            setDraftAddress(isInternalPage ? "" : (activeTab?.url ?? ""));
        }
    }, [activeTab?.id, activeTab?.url, editingTabId, isInternalPage]);

    const submit = (event: FormEvent<HTMLFormElement>): void => {
        event.preventDefault();
        const value = draftAddress.trim();
        if (!value) return;
        setEditingTabId(null);
        inputRef.current?.blur();
        api.navigation.navigate(value);
    };

    const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
        if (event.key !== "Escape") return;
        setEditingTabId(null);
        setDraftAddress(isInternalPage ? "" : (activeTab?.url ?? ""));
        inputRef.current?.blur();
    };

    return (
        <form aria-label="Address bar" className="photon-omnibox" onSubmit={submit}>
            <SiteSecurity isOpen={siteInfoOpen} onOpenChange={onSiteInfoOpenChange} tab={activeTab} />
            <Input
                ref={inputRef}
                aria-label="Search or enter address"
                autoComplete="off"
                className="photon-address-input"
                inputMode="url"
                placeholder="Search or enter address"
                value={isEditing ? draftAddress : isInternalPage ? "" : (activeTab?.url ?? "")}
                variant="secondary"
                onBlur={() => setEditingTabId(null)}
                onChange={(event) => {
                    setEditingTabId(activeTab?.id ?? null);
                    setDraftAddress(event.target.value);
                }}
                onFocus={(event) => {
                    const address = isInternalPage ? "" : (activeTab?.url ?? "");
                    setDraftAddress(address);
                    setEditingTabId(activeTab?.id ?? null);
                    if (address) event.currentTarget.select();
                }}
                onKeyDown={handleKeyDown}
            />
        </form>
    );
}
