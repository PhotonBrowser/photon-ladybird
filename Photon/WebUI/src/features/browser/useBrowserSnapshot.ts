// SPDX-License-Identifier: GPL-3.0-only
import { useEffect, useState } from "react";

import type { BrowserSnapshot, PhotonApi } from "../../types";

export function useBrowserSnapshot(api: PhotonApi): BrowserSnapshot {
    const [snapshot, setSnapshot] = useState(api.browser.state);
    useEffect(() => api.browser.onStateChange(setSnapshot), [api]);
    return snapshot;
}
