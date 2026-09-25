// SPDX-License-Identifier: GPL-3.0-only
import { useEffect, useRef, useState } from "react";

import type { PhotonApi } from "../../types";

export function useWindowTintOpacity(api: PhotonApi, persistedOpacity: number) {
    const [opacity, setOpacity] = useState(persistedOpacity);
    const timeout = useRef<number | null>(null);
    const isAdjusting = useRef(false);

    useEffect(() => {
        if (!isAdjusting.current) setOpacity(persistedOpacity);
    }, [persistedOpacity]);

    useEffect(
        () => () => {
            if (timeout.current !== null) window.clearTimeout(timeout.current);
        },
        [],
    );

    const update = (value: number): void => {
        setOpacity(value);
        if (timeout.current !== null) window.clearTimeout(timeout.current);
        timeout.current = window.setTimeout(() => {
            timeout.current = null;
            isAdjusting.current = false;
            api.settings.setWindowTintOpacity(value);
        }, 160);
    };

    const beginAdjusting = (): void => {
        isAdjusting.current = true;
    };

    const cancelAdjusting = (): void => {
        if (timeout.current !== null) return;
        isAdjusting.current = false;
        setOpacity(persistedOpacity);
    };

    return { opacity, update, beginAdjusting, cancelAdjusting };
}
