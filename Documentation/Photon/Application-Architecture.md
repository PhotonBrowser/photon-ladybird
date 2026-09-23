# Photon application architecture

This document defines ownership for Photon features and records the first incremental move toward a Rust-owned application core. Ladybird remains the browser engine; this architecture does not move Qt or Ladybird objects into Rust.

## Baseline ownership audit

| Area | Current owner | Responsibilities found |
| --- | --- | --- |
| UI / presentation | `Photon/WebUI` TypeScript + React | Browser chrome, internal-page presentation, transient focus/edit/popover state, typed `window.photon` API, command transport adapter, snapshot rendering |
| Application state and policy | `Photon/Rust` | Tab IDs/order/active identity, internal routes, preferences, config persistence, URL-input normalization, navigation command preparation, engine-reported page metadata, snapshot serialization |
| Command adapter and Ladybird integration | `Photon/Bridge/BrowserView.cpp` | Owns each `WebContentView`, performs loads/history operations, translates Ladybird callbacks into Rust state updates, updates page theme, adapts Rust state transitions to view creation/visibility/destruction |
| Command dispatch | `Photon/Bridge/PhotonWindow.cpp` | Matches typed `PhotonCommand` variants, routes browser/app operations to `BrowserView`, performs Qt window operations, clears native overlay capture for non-capture commands |
| Trusted UI transport | `Photon/Bridge/ChromeSurface.cpp`, `PhotonCommandTransport.cpp` | Loads trusted WebUI, carries initial state/events, consumes the temporary navigation transport, validates its shape/arguments, produces typed Photon commands |
| Window/input integration | `Photon/Bridge/WindowScene.cpp`, `PhotonWindow.cpp` | Composes separate trusted/untrusted WebContent views, forwards input, starts Qt window operations, stores the minimal overlay input-capture bit |
| Browser engine | Ladybird WebContent / LibWebView | Web standards, actual URL/title/loading/history, page rendering, engine events and view behavior |

### Existing command and state paths

```text
React API → typed TypeScript command → temporary navigation transport
    → ChromeSurface decoder → Window dispatcher
    → BrowserView → Rust state methods → Ladybird effects

Ladybird callbacks → BrowserView → Rust state → snapshot JSON
    → ChromeSurface photon-state event → React
```

The Rust `BrowserState` already makes the core tab and preference decisions. However, `BrowserView` currently reconstructs tab transition effects in C++: it discovers removed tabs by comparing Rust tab-ID lists, determines whether the active tab changed, and decides which Qt signals and view synchronization to perform. Those are application transition decisions; managing `WebContentView` objects and applying the results are integration work.

`Window::dispatch_command` is an adapter, not the long-term application state machine. Qt window operations, transparent scene composition, input forwarding, and Ladybird callbacks remain appropriately native. React owns only presentation and transient UI state.

## Rust application core

`PhotonApp` is the Photon-owned application coordinator. It owns `BrowserState`, including preferences and persisted configuration, and accepts domain commands. It returns a compact `AppEffects` value describing state changes and native page-view lifecycle effects. It has no Qt or Ladybird dependency.

```text
PhotonApp::dispatch(AppCommand)
    ├── mutates Rust-owned tab/preferences state
    └── returns AppEffects
              ├── create/remove native page view
              ├── active tab changed
              ├── snapshot/state changed
              └── theme changed
```

The first migration uses Rust dispatch for tab lifecycle and preferences. Navigation normalization and `BrowserCommand` preparation remain in the existing Rust browser model. C++ consumes the resulting commands/effects and performs the corresponding Ladybird or Qt operations. Window controls and overlay capture stay native because they represent platform calls and scene input routing, not persistent browser application state.

The current C ABI remains a narrow same-process boundary. It carries typed command kinds, scalar IDs, an ID slice for reorder, and fixed-layout effect flags/IDs. Rust owns no native pointer. Page-view creation, deletion, loading, history traversal, and theme application remain in C++.

## Future feature placement

```text
Is it React presentation or transient interaction state?
    → TypeScript / React

Is it Photon application state, policy, or a domain transition?
    → Photon Rust application core

Must it manipulate a Ladybird C++ object or consume an engine callback?
    → thin Photon C++ adapter

Must it call Qt or a native window-system API?
    → thin Photon C++ / Qt adapter

Is it a web-platform behavior?
    → Ladybird in its upstream architecture and language
```

## Migration candidates

Prioritized remaining Photon-owned candidates:

1. **Navigation coordination effects:** Rust already normalizes input and prepares typed navigation commands. Over time, let the application core return a unified `Navigate`/`Reload`/`HistoryTraversal` effect instead of keeping browser command coordination split between `BrowserView` methods and Rust preparation. C++ still performs the Ladybird load/traversal.
2. **Session model and restore policy:** add tab/session persistence, restore decisions, and lifecycle policy to `PhotonApp`; leave URL loading and page-view construction in `BrowserView`.
3. **Application-level command routing:** move Photon command policy and state transitions behind Rust app commands. Keep the C++ dispatcher limited to Qt commands and executing typed effects; the trusted transport and Qt window commands are not Rust responsibilities.
4. **Window state model only if product behavior needs it:** Rust may own requested window state or policy if Photon adds persistence/coordination. Qt remains the source for actual minimized/maximized/native geometry state and executes platform operations.

Do not migrate `WebContentView` ownership, URL loading/history APIs, title/loading/favicon callbacks, view composition, focus routing, pointer/wheel forwarding, native system move, or OS window controls. They directly integrate with Ladybird or Qt.

## C ABI assessment

The present C ABI is appropriate for this in-process application: it has one opaque app-state handle, narrow typed commands, and fixed-layout effects. It avoids Rust dependencies on Qt/Ladybird and has no callback ownership or cross-thread handles. This pass consolidates Photon tab/preferences mutations into one application dispatch operation instead of adding another FFI framework.

If commands and effects grow substantially, use a versioned command/effect batch or a generated C ABI binding rather than adding one export per UI action. A new FFI technology is not justified by the current surface. Any future transport replacement for `photon-command://` should call the same Photon application command boundary and leave this state/effect contract unchanged.
