// SPDX-License-Identifier: GPL-3.0-only
import { memo } from "react";

import logo from "../assets/photon-logo.svg";

export const NewTabPage = memo(function NewTabPage(): React.JSX.Element {
    return (
        <main aria-labelledby="new-tab-title" className="photon-internal-page photon-new-tab-page">
            <div className="photon-new-tab-content">
                <header className="photon-new-tab-brand">
                    <img alt="" className="photon-brand-logo" height={40} src={logo} width={40} />
                    <h1 id="new-tab-title">Photon</h1>
                </header>
            </div>
        </main>
    );
});
