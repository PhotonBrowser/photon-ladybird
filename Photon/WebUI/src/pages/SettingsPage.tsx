import { ListBox, Select } from "@heroui/react";

import type { BrowserSnapshot, PhotonApi, ThemeMode } from "../types";

interface SettingsPageProps {
    api: PhotonApi;
    snapshot: BrowserSnapshot;
}

const themeOptions: Array<{ id: ThemeMode; label: string }> = [
    { id: "system", label: "System" },
    { id: "light", label: "Light" },
    { id: "dark", label: "Dark" },
];

export function SettingsPage({ api, snapshot }: SettingsPageProps): React.JSX.Element {
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
                                selectedKey={snapshot.themeMode}
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
