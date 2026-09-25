// SPDX-License-Identifier: GPL-3.0-only
import { ListBox, Select } from "@heroui/react";

import type { BrowserSnapshot, PhotonApi, ThemeMode } from "../types";

interface SettingsPageProps {
    api: PhotonApi;
    snapshot: BrowserSnapshot;
    themeDropdownOpen: boolean;
    onThemeDropdownOpenChange(open: boolean): void;
}

const themeOptions: Array<{ id: ThemeMode; label: string }> = [
    { id: "system", label: "System" },
    { id: "light", label: "Light" },
    { id: "dark", label: "Dark" },
];

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
                    <section aria-labelledby="appearance-title" className="photon-settings-group">
                        <div className="photon-settings-group-header">
                            <div className="photon-settings-copy">
                                <h2 id="appearance-title">Appearance</h2>
                                <p>Choose Photon’s appearance.</p>
                            </div>
                            <Select
                                aria-label="Theme"
                                className="photon-settings-select"
                                isOpen={themeDropdownOpen}
                                selectedKey={snapshot.themeMode}
                                onOpenChange={onThemeDropdownOpenChange}
                                onSelectionChange={(key) => {
                                    if (key === "system" || key === "light" || key === "dark") {
                                        api.settings.setTheme(key as ThemeMode);
                                    }
                                }}
                            >
                                <Select.Trigger className="photon-settings-select-trigger">
                                    <Select.Value />
                                    <Select.Indicator className="photon-settings-select-indicator" />
                                </Select.Trigger>
                                <Select.Popover className="photon-popover photon-settings-select-popover">
                                    <ListBox>
                                        {themeOptions.map((option) => (
                                            <ListBox.Item
                                                className="photon-settings-select-item"
                                                id={option.id}
                                                key={option.id}
                                            >
                                                {option.label}
                                            </ListBox.Item>
                                        ))}
                                    </ListBox>
                                </Select.Popover>
                            </Select>
                        </div>
                        <label className="photon-settings-toggle-row">
                            <span className="photon-settings-copy">
                                <span className="photon-settings-toggle-title">Force dark web pages</span>
                                <span className="photon-settings-toggle-description">
                                    Darken sites that do not provide their own dark theme.
                                </span>
                            </span>
                            <input
                                checked={snapshot.forceDarkPages}
                                onChange={(event) => api.settings.setForceDarkPages(event.currentTarget.checked)}
                                type="checkbox"
                            />
                        </label>
                        <label className="photon-settings-toggle-row">
                            <span className="photon-settings-copy">
                                <span className="photon-settings-toggle-title">
                                    Dim background for popovers and dropdowns
                                </span>
                                <span className="photon-settings-toggle-description">
                                    Darken the page behind open overlays.
                                </span>
                            </span>
                            <input
                                checked={snapshot.dimOverlays}
                                onChange={(event) => api.settings.setDimOverlays(event.currentTarget.checked)}
                                type="checkbox"
                            />
                        </label>
                        <div className="photon-settings-range-row">
                            <label className="photon-settings-copy" htmlFor="window-tint-opacity">
                                <span className="photon-settings-toggle-title">Window tint</span>
                                <span className="photon-settings-toggle-description">
                                    Adjust the color overlay. Blur strength is controlled by your Wayland compositor.
                                </span>
                            </label>
                            <div className="photon-settings-range-control">
                                <input
                                    id="window-tint-opacity"
                                    max="100"
                                    min="0"
                                    onChange={(event) =>
                                        api.settings.setWindowTintOpacity(Number(event.currentTarget.value))
                                    }
                                    type="range"
                                    value={snapshot.windowTintOpacity}
                                />
                                <output htmlFor="window-tint-opacity">{snapshot.windowTintOpacity}%</output>
                            </div>
                        </div>
                    </section>
                    <section aria-labelledby="privacy-title" className="photon-settings-group">
                        <div className="photon-settings-copy">
                            <h2 id="privacy-title">Privacy and security</h2>
                            <p>Website content stays isolated from Photon’s browser controls.</p>
                            <p>Photon uses Ladybird for page loading and rendering.</p>
                        </div>
                    </section>
                </div>
            </div>
        </main>
    );
}
