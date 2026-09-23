// SPDX-License-Identifier: GPL-3.0-only
import type { ThemeMode } from "../types";

export type PhotonCommand =
    | { kind: "navigate"; url: string }
    | { kind: "back" }
    | { kind: "forward" }
    | { kind: "reload" }
    | { kind: "new-tab" }
    | { kind: "open-settings" }
    | { kind: "select-tab"; tabId: string }
    | { kind: "close-tab"; tabId: string }
    | { kind: "reorder-tabs"; tabIds: string[] }
    | { kind: "capture"; region: "browser-menu" | "site-info"; open: boolean }
    | { kind: "set-theme"; mode: ThemeMode }
    | { kind: "set-force-dark-pages"; enabled: boolean }
    | { kind: "set-dim-overlays"; enabled: boolean }
    | { kind: "window-minimize" }
    | { kind: "window-toggle-maximize" }
    | { kind: "window-close" }
    | { kind: "window-start-system-move" };

export interface PhotonCommandTransport {
    dispatch(command: PhotonCommand): void;
}

/** The navigation transport is temporary; callers depend only on PhotonCommand. */
export function createNavigationCommandTransport(enabled: boolean): PhotonCommandTransport {
    return {
        dispatch(command) {
            if (!enabled) return;
            const { name, value } = serializeCommand(command);
            const query = value === undefined ? "" : `?value=${encodeURIComponent(value)}`;
            window.location.href = `photon-command://${name}${query}`;
        },
    };
}

function serializeCommand(command: PhotonCommand): { name: string; value?: string } {
    switch (command.kind) {
        case "navigate":
            return { name: command.kind, value: command.url };
        case "select-tab":
        case "close-tab":
            return { name: command.kind, value: command.tabId };
        case "reorder-tabs":
            return { name: command.kind, value: command.tabIds.join(",") };
        case "capture":
            return { name: command.kind, value: `${command.region}:${command.open ? "open" : "close"}` };
        case "set-theme":
            return { name: command.kind, value: command.mode };
        case "set-dim-overlays":
        case "set-force-dark-pages":
            return { name: command.kind, value: String(command.enabled) };
        case "window-start-system-move":
            return { name: "window-drag" };
        case "window-minimize":
        case "window-toggle-maximize":
        case "window-close":
        case "back":
        case "forward":
        case "reload":
        case "new-tab":
        case "open-settings":
            return { name: command.kind };
        default:
            return assertNever(command);
    }
}

function assertNever(value: never): never {
    throw new Error(`Unhandled Photon command: ${JSON.stringify(value)}`);
}
