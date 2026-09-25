# Photon application architecture

This document records the Photon application ownership migration and its current trusted bridge. Ladybird remains the web engine, and Qt remains the platform/window integration toolkit.

## Before: ownership and command path (historical)

```text
React / TypeScript
    │ typed Photon API and TypeScript command union
    ▼
PhotonCommandTransport
    │ temporary photon-command:// adapter
    ▼
ChromeSurface decoder and allowlist
    ▼
PhotonWindow::dispatch_command
    ├── BrowserView ── narrow Rust C ABI ── BrowserState
    │                    └── Ladybird WebContentView operations
    ├── WindowScene ── Qt input capture
    └── Qt window controls

Ladybird callbacks → BrowserView → Rust BrowserState → snapshot
    → ChromeSurface photon-state event → React
```

| Responsibility | Before owner | Classification |
| --- | --- | --- |
| Browser chrome, settings page, overlays, transient editing/focus | React / TypeScript | UI |
| Tab identity, ordering, active tab, internal routes | Rust `BrowserState`, initiated from C++ methods | Application logic and state |
| Theme and overlay appearance preferences, config serialization | Rust `BrowserState` and config module | Application state and persistence |
| Command URL decoding and validation | Photon C++ `ChromeSurface` transport decoder | Transport and security boundary |
| Native command routing | Photon C++ `Window::dispatch_command` | Integration routing, with duplicated application dispatch decisions |
| Per-tab `WebContentView` objects, page callbacks, actual navigation/load/history operations | Photon C++ `BrowserView` + Ladybird APIs | Browser-engine integration |
| Page URL/title/loading/history truth | Ladybird callbacks, represented in Rust snapshots | Engine state |
| Window controls and native system move | Photon C++ `Window` + Qt | Platform/window integration |
| Overlay input capture and pointer/wheel forwarding | Photon C++ `WindowScene` + Qt | Native input/composition integration |
| Initial snapshot and state update delivery | Rust serialization; C++ bootstrap/event dispatch | State transport |

Rust already owns browser facts and persistence. Before this migration, however, C++ made tab lifecycle decisions around those facts. In particular, `BrowserView::close_tab` took two tab-ID snapshots and diffed them to infer which native view to destroy, while `BrowserView` itself decided which state signals to emit after each mutation.

## After: Rust application decisions and native effects

```text
React / TypeScript
    │ typed Photon API
    ▼
PhotonCommandTransport → trusted embedder messaging
    ▼
ChromeSurface → typed PhotonCommand → PhotonWindow dispatcher
    ├── browser navigation ──► BrowserView ──► Rust navigation model
    │                                      └──► Ladybird WebContentView
    ├── tab/preferences ─────► BrowserView adapter
    │                              │ PhotonAppCommand
    │                              ▼
    │                         Rust PhotonApp
    │                              │ PhotonAppEffects
    │                              ▼
    │                         BrowserView applies view/signal effects
    ├── overlay input capture ─► WindowScene / Qt
    └── native window controls ► Qt window

Ladybird callbacks → BrowserView → Rust application state/snapshot
    → ChromeSurface trusted embedder event → React
```

`PhotonApp` now owns decisions for create/open/select/close/reorder tab commands and theme/dimming preference updates. Its typed `AppCommand` accepts domain operations, and `AppEffects` describes state transitions for the adapter, including the tab ID to create or remove and whether active content changed. One C ABI dispatch function carries this operation/effect boundary. C++ performs the corresponding `WebContentView` allocation/destruction, active-view composition, Qt preferred-color-scheme update, and UI state notifications.

The C++ adapter no longer reconstructs tab removal by enumerating Rust state before and after a close. Closing the last tab is represented as replacement of active content under the same tab ID, so the existing native view remains allocated while React receives the new-tab state.

Navigation, URL normalization, and history command preparation remain in the existing Rust `BrowserState` path. Ladybird still performs the page load, reload, and history traversal. Qt window controls, system move, overlay capture, event routing, and WebContent view lifetime remain native integration responsibilities.

## Remaining ownership audit

`PhotonApp` validates explicit tab-close requests and decides whether to reset the final tab in place or ask the native adapter to request a Ladybird close. For non-final tabs, Rust commits removal only when the `PageClosed` observation arrives. Rust also owns favicon URL metadata after C++ encodes the Ladybird bitmap. No current tab or favicon policy remains in `BrowserView`.

Remaining C++ behavior is integration: `BrowserView` maps a Rust close intent to `WebContentView::request_close`, receives Ladybird's close callback, encodes favicon bitmaps, and forwards page observations. `PhotonWindow` detects Qt key combinations and performs Qt window operations. Shortcut-to-command policy is already routed through Rust.

Session restoration, multi-window ownership, and richer application configuration are not implemented sufficiently to justify speculative core abstractions now. React interaction state remains UI-owned.

## Rust/C++ boundary

The existing C ABI remains appropriate for the small number of synchronous calls across this boundary. It uses an opaque per-window Rust state allocation, borrowed UTF-8 snapshot/query values, one application command/effect dispatch, and one partial page observation operation. Tab IDs and values cross the boundary; Rust does not own Qt or Ladybird pointers.

Ladybird callbacks update URL, title, favicon, loading, and navigation capabilities through one tagged partial `photon_app_observe_page` call. The adapter supplies one observation per engine callback, so no artificial atomic multi-field snapshot is imposed.

The application command kind is a narrow numeric C representation decoded immediately into a Rust `AppCommand`; it is not a general string command channel. Invalid kinds, preference values, and invalid reorder buffers are rejected. Keep command definitions adjacent in `PhotonCore.h` and Rust FFI code. A generated binding or another interoperability technology is not warranted by this pass; reassess only if ABI drift or data volume becomes a demonstrated maintenance problem.

## Development decision tree

For a new Photon feature:

1. Is it presentation, interaction, or transient UI state? Put it in React/TypeScript.
2. Is it Photon application behavior, persistent state, command policy, tab/session logic, or feature coordination? Put it in Rust.
3. Must it manipulate a Ladybird C++ object or receive an engine callback? Keep the smallest adapter in Photon C++.
4. Must it call Qt or a native platform API? Keep that call in Photon C++.
5. Is the behavior part of web-platform/engine semantics? Implement it in Ladybird's architecture and keep any Photon-specific engine change in the registered patch series.

Do not put Photon product policy in Ladybird, and do not make C++ the default home for new Photon application logic.

## Navigation and observation migration (September 2026)

The typed native dispatcher now sends navigation, reload, and history requests through `photon_app_dispatch`. `PhotonApp` chooses the active tab, normalizes a submitted address, updates the pending Photon snapshot, and returns a typed navigation intent with a tab ID. Internal Photon routes update Rust state without a Ladybird load. `BrowserView` executes the intent against its tab-ID-to-view map. This keeps the engine-reported URL authoritative after redirects and history traversal.

Native shortcuts still use Qt to recognize platform key combinations. They now use the same Rust command path for tab creation, adjacent selection, reload, history traversal, and focus-address intent. Tab close remains coordinated with Ladybird's asynchronous `request_close` callback; native waits for that callback before asking Rust to remove the tab. The Qt window and overlay-capture commands remain native operations.

Ladybird emits URL, title, loading, and history capabilities independently. The adapter sends those events through one `photon_app_observe_page` operation with a tagged partial observation. Rust updates its snapshot and preserves the special handling of internal pages. The favicon remains a separate image/data-URL update because it is prepared by the native bitmap encoder.

`BrowserView` still owns the Rust handle and the native view map. Its `apply_app_effects` switch is the effect executor for now: it creates and destroys views, performs tab-specific navigation, applies appearance, and publishes state signals. A separate executor class would add indirection without splitting a concrete native lifetime boundary today. The view map caches object associations, not tab order or active-tab policy.

The C ABI remains small enough for handwritten structs and functions: one application dispatch, one partial page observation (including favicon metadata), and snapshot queries. Effect strings borrow Rust storage until the next state mutation, so the C++ executor copies a target URL before calling Ladybird, whose callbacks may synchronously reenter the adapter. This does not call for an FFI framework.

## Close requests and favicon observations

For a requested tab close, `PhotonApp` checks the ID and tab count. A nonexistent ID is rejected; closing the final tab resets its Rust record in place; closing another tab returns a native close request without mutating the tab list. `BrowserView` executes that request with `WebContentView::request_close`. Its `on_close` callback sends `PageClosed(tab_id)` to Rust, which then removes the tab and chooses the next active tab. This keeps asynchronous engine close lifecycle authoritative while removing the tab-count decision from C++.

Favicon bitmap access and PNG/base64 conversion remain in `BrowserView`, where Ladybird graphics types are available. The resulting data URL is sent as `PageObservation::Favicon`; Rust stores or clears the metadata and includes it in the snapshot. Favicon metadata therefore uses the same page-observation operation as other engine facts.

## Current transport boundary

`PhotonCommandTransport` owns typed command dispatch and event subscriptions. TypeScript sends commands over Ladybird's trusted embedder channel; `PhotonCommandTransport.cpp` validates structured messages against the native allowlist, and `ChromeSurface` connects the callback to the existing typed dispatcher. State and focus events return over the same channel. No application command, React API, Rust effect, or native executor depends on transport details. The channel exists only on the trusted chrome view and is bound to its authorized committed top-level document. See [Bridge Audit](Bridge-Audit.md) for Ladybird lifecycle and security details.
