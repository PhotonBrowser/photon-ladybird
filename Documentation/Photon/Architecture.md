# Photon architecture

Photon is an overlay on Ladybird. Ladybird remains responsible for the web platform, rendering, networking, media, IPC, sandboxing, and helper processes.

```text
User command

React/TypeScript toolbar
    ▼
Rust BrowserCommand / BrowserState
    ▼
BrowserView adapter
    ▼
LibWebView
    ▼
WebContent

Engine event

WebContent / LibWebView
    ▼
BrowserView callback
    ▼
Rust BrowserState
    ▼
React component state
```

## Responsibilities

- `Photon/WebUI` owns visual composition and address-field editing state. It renders the navigation toolbar and privileged overlays; it forwards commands without parsing URLs or maintaining history. `Photon/UI` remains the deprecated QML compatibility frontend during the migration.
- `Photon/Rust` owns the product-facing state for the attached surface: current URL, title, loading state, and back/forward availability. It normalises conservative hostname input and produces typed navigate, reload, back, and forward commands. It does not duplicate Ladybird's history.
- `Photon/Bridge` adapts Rust and Ladybird to Qt. `BrowserView` owns one opaque Rust state handle and wraps one `WebContentView`. It consumes typed commands synchronously, subscribes to Ladybird callbacks and navigation actions, updates Rust, then emits narrow Qt property notifications.
- Ladybird's existing `WebContentView` handles rendering and input. Photon compiles that frontend implementation directly from `UI/Qt`; it does not copy or fork it.

Ladybird is authoritative for the actual current URL, including redirects, links, same-document navigation, and history traversal. It is also authoritative for page titles, loading, and whether history traversal is available. Rust represents that engine-reported state for Photon; a submitted address is never treated as the final URL.

The C ABI uses one opaque `PhotonBrowserState` allocation per view. Borrowed strings are copied by C++ before another Rust mutation, commands are consumed synchronously, and the adapter destroys the Ladybird view before freeing callback state. No Ladybird pointer crosses into Rust.

The current Ladybird Qt surface is a `QWidget`/`QRhiWidget`, not a `QQuickItem`. The React Web UI therefore uses a small QWidget scene coordinator at the integration edge. The deprecated QML path still uses `QQuickWidget` for compatibility; it is not part of the Web UI composition model.

```text
PhotonWindow
├── WebContentView
│   └── Photon React chrome document
├── Ladybird WebContentView
│   └── QWidget / QRhiWidget surface
└── transient Qt Quick windows
    └── UI that must overlap web content
```

The Web UI uses two sibling Ladybird `WebContentView` surfaces: the page surface is laid out below the toolbar, while the transparent full-window chrome surface is painted above it. `WindowScene` forwards pointer and wheel input outside the chrome capture regions to the page. This is Photon window integration policy, not Rust state or `BrowserView` responsibility.

Moving Ladybird rendering to a Qt Quick-native surface could remove this boundary in the future, but it is a separate integration project and is not part of the current architecture.

One view and one browser state are created today. A future tab model should associate one state and browser-view instance with each tab, while tabs, workspaces, preferences, and session state belong in Rust. Native dialogs and rendering/input plumbing remain in the C++ adapter. Presentation and animation live in React/CSS; the QML equivalent remains only for the deprecated fallback.

## Web UI experiment

The QML frontend is now a fallback/reference path. `./photon run` creates a `WindowScene` containing two independent Ladybird web contexts:

```text
Photon window
└── WindowScene
    ├── ChromeSurface  bundled Photon React/TypeScript
    └── PageSurface    ordinary website
```

The chrome surface is full-window and transparent outside its capture regions, so its HTML/CSS popover can overlap the page while `WindowScene` forwards pointer and wheel events elsewhere to the page. The DOMs are separate. Rust still owns product state, C++ only coordinates native views, and Ladybird remains authoritative for navigation and rendering. Details and current limitations are in [WebUI.md](WebUI.md).
