# Photon command bridge audit

This records the bridge before transport restructuring. It describes the trusted React WebUI path; ordinary page views are separate Ladybird `WebContentView` instances.

## Current command flow

```text
React components
    │ typed public methods in PhotonApi
    ▼
Photon/WebUI/src/main.tsx
    │ string command name + optional string value
    │ window.location.href = photon-command://…
    ▼
ChromeSurface::handle_navigation_request
    │ top-level navigation hook, installed on ChromeSurface's view only
    │ URL shape + command + argument validation
    ▼
PhotonWindow callback
    ├── BrowserView ── C ABI ── Rust BrowserState / BrowserCommand ── Ladybird WebContentView
    ├── Rust-backed preference methods via BrowserView ── Qt color scheme / React state event
    ├── WindowScene overlay capture bit
    └── Qt QWidget/QWindow window operations
```

`ChromeSurface` consumes all `photon-command:` navigation attempts, even invalid ones. Its Ladybird navigation callback is called only for top-level navigation requests. Other top-level navigation from the trusted chrome is canceled; development mode additionally permits the pinned `http://127.0.0.1:5173` origin. The ordinary page view has neither this callback nor the chrome bootstrap script.

## Public trusted API

`Photon/WebUI/src/types.ts` defines the frontend API; `main.tsx` creates the object and assigns it to `window.photon` in the WebUI entry document.

| API group | Methods / values | Current destination |
| --- | --- | --- |
| `navigation` | `navigate`, `back`, `forward`, `reload` | `BrowserView`; Rust prepares typed `BrowserCommand`; C++ performs the corresponding Ladybird load/history operation |
| `tabs` | `create`, `openSettings`, `select`, `close`, `reorder` | `BrowserView`; typed Rust C ABI methods mutate tab state; C++ creates/selects/destroys page views |
| `settings` | `setTheme`, `setDimOverlays` | `BrowserView` and Rust state; theme is also applied to current Ladybird views |
| `ui` | `setCaptureRegion` | `WindowScene` stores only the overlay input-capture bit |
| `window` | `platform`, `beginDrag`, `minimize`, `toggleMaximize`, `close` | `PhotonWindow` / Qt; `platform` is bootstrap metadata |
| `browser` | `state`, `onStateChange` | React reads the latest snapshot and subscribes to `photon-state` |

Commands currently encoded as URL host/value pairs are `navigate`, `back`, `forward`, `reload`, `new-tab`, `open-settings`, `select-tab`, `close-tab`, `reorder-tabs`, `set-theme`, `set-dim-overlays`, `capture`, `window-minimize`, `window-toggle-maximize`, `window-close`, and `window-drag`. Tab identifiers, reorder lists, theme values, booleans, and capture-region transitions are checked by `ChromeSurface::is_allowed_command` before the callback.

The URL is generated only in `Photon/WebUI/src/main.tsx` today. It is intercepted only in `Photon/Bridge/ChromeSurface.cpp`. No other `photon-command:` producer/consumer or navigation-based IPC substitute was found in Photon source. Normal page navigation travels through `BrowserView` and Rust's navigation preparation, then into Ladybird's normal `WebContentView::load` path; it does not use the command URL.

## Rust and native ownership

Rust owns persistent browser state: tabs, IDs/order, internal routes, preferences, and the page metadata snapshot. Its `BrowserCommand` covers URL navigation and history operations; the C ABI also exposes typed functions for tab and preference operations. C++ owns Ladybird view pointers, translates engine callbacks into Rust state updates, and applies browser commands to `WebContentView`. Qt window operations and the transient overlay input-capture bit are native concerns and currently bypass Rust state.

Ladybird remains authoritative for actual web URL, title, loading, favicon, and history availability. `BrowserView` receives per-view callbacks, updates Rust, and emits Qt signals. The native side returns a serialized Rust snapshot through `ChromeSurface::update_state`, which dispatches a `photon-state` DOM event. Initial state is injected while loading the trusted bundled document. A separate fixed `photon-focus-address` event sends a keyboard-shortcut notification from native code to React.

## Trust boundary review

- Production creates a separate trusted chrome `WebContentView` and untrusted page `WebContentView`. Only the chrome view is loaded with the Photon bundle and given `on_navigation_request`.
- The Ladybird hook in the registered navigation patch applies to top-level requests. Invalid command URLs and other top-level destinations are canceled before replacing the chrome document.
- The API is assigned by trusted application JavaScript, not by a global Ladybird or WebContent injection. Ordinary pages therefore do not receive `window.photon` or a native object.
- A page cannot access the chrome view's JavaScript context through Photon composition; the two views are separate documents/contexts. No general frame or website bridge was found.
- Production initial state is currently interpolated into an inline script. Since page title and URL fields can contain attacker-controlled text, this needs safe data encoding before it enters the trusted script context.
- The development bridge deliberately trusts the pinned local Vite origin. It is a developer-controlled context and must remain unavailable to arbitrary origins.

## Temporary transport and migration boundary

`photon-command://` is only a transport shim: React callers already use domain methods, and `BrowserView` already calls typed Rust APIs. Replacing it should affect the TypeScript command transport and the native adapter at `ChromeSurface`, not React components or Rust's browser state model. A dedicated native JS binding would still need a Photon-owned, trusted-context-scoped way to deliver typed messages into the Qt/native dispatcher and return events; Ladybird currently exposes a top-level navigation interception callback, not that binding.
