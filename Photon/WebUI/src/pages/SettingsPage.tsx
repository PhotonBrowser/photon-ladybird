// SPDX-License-Identifier: GPL-3.0-only
import type { BrowserSnapshot, PhotonApi } from "../types";
import { SettingsAppearanceSection } from "../features/settings/SettingsAppearanceSection";
import { SettingsPrivacySection } from "../features/settings/SettingsPrivacySection";

interface SettingsPageProps {
    api: PhotonApi;
    snapshot: BrowserSnapshot;
    themeDropdownOpen: boolean;
    onThemeDropdownOpenChange(open: boolean): void;
}

export function SettingsPage({
    api,
    snapshot,
    themeDropdownOpen,
    onThemeDropdownOpenChange,
}: SettingsPageProps): React.JSX.Element {
    return (
        <main aria-labelledby="settings-title" className="photon-internal-page photon-settings-page">
            <div className="photon-settings-content">
                <header className="photon-settings-heading">
                    <h1 id="settings-title">Settings</h1>
                    <p>Make Photon work the way you do.</p>
                </header>
                <div className="photon-settings-panel">
                    <SettingsAppearanceSection
                        api={api}
                        snapshot={snapshot}
                        themeDropdownOpen={themeDropdownOpen}
                        onThemeDropdownOpenChange={onThemeDropdownOpenChange}
                    />
                    <SettingsPrivacySection />
                </div>
            </div>
        </main>
    );
}
