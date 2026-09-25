// SPDX-License-Identifier: GPL-3.0-only
import type { ThemeMode } from "../types";

export type PhotonCommand =
    | { kind: "navigate"; url: string }
    | { kind: "back" }
    | { kind: "forward" }
    | { kind: "reload" }
    | { kind: "focus-address" }
    | { kind: "ui-ready" }
    | { kind: "new-tab" }
    | { kind: "open-settings" }
    | { kind: "select-tab"; tabId: string }
    | { kind: "close-tab"; tabId: string }
    | { kind: "reorder-tabs"; tabIds: string[] }
    | { kind: "capture"; region: "browser-menu" | "site-info"; open: boolean }
    | { kind: "set-theme"; mode: ThemeMode }
    | { kind: "set-force-dark-pages"; enabled: boolean }
    | { kind: "set-dim-overlays"; enabled: boolean }
    | { kind: "set-window-tint-opacity"; opacity: number }
    | { kind: "window-minimize" }
    | { kind: "window-maximize" }
    | { kind: "window-toggle-maximize" }
    | { kind: "window-close" }
    | { kind: "set-titlebar-drag-region"; enabled: boolean };

export interface PhotonCommandTransport {
    dispatch(command: PhotonCommand): void;
    subscribe(listener: (event: PhotonTransportEvent) => void): () => void;
}

export type PhotonTransportEvent =
    | { type: "state"; detail: unknown }
    | { type: "page-tooltip"; detail: { text: string; x: number; y: number } }
    | { type: "page-tooltip-clear" }
    | { type: "page-crashed" }
    | { type: "chrome-crashed" }
    | { type: "focus-address" }
    | { type: "blur-address" };

/** Dispatch Photon commands only through the trusted native messaging channel. */
export function createPhotonCommandTransport(): PhotonCommandTransport {
    const listeners = new Set<(event: PhotonTransportEvent) => void>();
    return {
        dispatch(command) {
            const nativeChannel = window.embedderMessaging;
            if (!nativeChannel)
                throw new Error("Photon command API is unavailable without its trusted native messaging channel.");
            const { type, payload } = serializeNativeCommand(command);
            nativeChannel.postMessage(type, payload);
        },
        subscribe(listener) {
            listeners.add(listener);
            const onNativeMessage = (): void => {
                const channel = window.embedderMessaging;
                if (!channel) return;
                let messages: unknown;
                try {
                    messages = JSON.parse(channel.receiveMessages()) as unknown;
                } catch {
                    return;
                }
                if (!Array.isArray(messages)) return;
                for (const value of messages) {
                    if (!value || typeof value !== "object") continue;
                    const message = value as Record<string, unknown>;
                    if (typeof message.type !== "string") continue;
                    if (message.type === "state") {
                        for (const currentListener of listeners)
                            currentListener({ type: "state", detail: message.payload });
                    } else if (message.type === "focus-address") {
                        for (const currentListener of listeners) currentListener({ type: "focus-address" });
                    } else if (message.type === "blur-address") {
                        for (const currentListener of listeners) currentListener({ type: "blur-address" });
                    }
                }
            };
            document.addEventListener("TrustedEmbedderMessageAvailable", onNativeMessage);
            onNativeMessage();
            const handlers: Array<[string, (event: Event) => void]> = [
                [
                    "photon-page-tooltip",
                    (event) => {
                        const detail = (event as CustomEvent<unknown>).detail;
                        if (!isPageTooltip(detail)) return;
                        listener({ type: "page-tooltip", detail });
                    },
                ],
                ["photon-page-tooltip-clear", () => listener({ type: "page-tooltip-clear" })],
                ["photon-page-crashed", () => listener({ type: "page-crashed" })],
                ["photon-chrome-crashed", () => listener({ type: "chrome-crashed" })],
            ];
            for (const [name, handler] of handlers) window.addEventListener(name, handler);
            return () => {
                listeners.delete(listener);
                document.removeEventListener("TrustedEmbedderMessageAvailable", onNativeMessage);
                for (const [name, handler] of handlers) window.removeEventListener(name, handler);
            };
        },
    };
}

function isPageTooltip(value: unknown): value is { text: string; x: number; y: number } {
    if (!value || typeof value !== "object") return false;
    const detail = value as Record<string, unknown>;
    return typeof detail.text === "string" && typeof detail.x === "number" && typeof detail.y === "number";
}

function serializeNativeCommand(command: PhotonCommand): { type: string; payload: Record<string, unknown> } {
    switch (command.kind) {
        case "navigate":
            return { type: command.kind, payload: { url: command.url } };
        case "select-tab":
        case "close-tab":
            return { type: command.kind, payload: { tabId: command.tabId } };
        case "reorder-tabs":
            return { type: command.kind, payload: { tabIds: command.tabIds } };
        case "capture":
            return { type: command.kind, payload: { region: command.region, open: command.open } };
        case "set-theme":
            return { type: command.kind, payload: { mode: command.mode } };
        case "set-dim-overlays":
        case "set-force-dark-pages":
            return { type: command.kind, payload: { enabled: command.enabled } };
        case "set-window-tint-opacity":
            return { type: command.kind, payload: { opacity: command.opacity } };
        case "set-titlebar-drag-region":
            return { type: command.kind, payload: { enabled: command.enabled } };
        case "window-minimize":
        case "window-maximize":
        case "window-toggle-maximize":
        case "window-close":
        case "back":
        case "forward":
        case "reload":
        case "focus-address":
        case "ui-ready":
        case "new-tab":
        case "open-settings":
            return { type: command.kind, payload: {} };
        default:
            return assertNever(command);
    }
}

function assertNever(value: never): never {
    throw new Error(`Unhandled Photon command: ${JSON.stringify(value)}`);
}
