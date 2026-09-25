// SPDX-License-Identifier: GPL-3.0-only

export function SettingsPrivacySection(): React.JSX.Element {
    return (
        <section aria-labelledby="privacy-title" className="photon-settings-group">
            <div className="photon-settings-copy">
                <h2 id="privacy-title">Privacy and security</h2>
                <p>Website content stays isolated from Photon’s browser controls.</p>
                <p>Photon uses Ladybird for page loading and rendering.</p>
            </div>
        </section>
    );
}
