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

- `Photon/WebUI` owns browser chrome composition, address editing, popovers, and internal-page presentation. It sends typed commands and renders snapshots; it does not own browser or tab state. `Photon/UI` remains the deprecated QML compatibility frontend.
- `Photon/Rust` owns tab order, active-tab identity, internal-page routes, appearance preferences, and each tab's URL, title, loading state, and history capabilities. Ladybird remains authoritative for the history of every web tab.
- `Photon/Bridge` adapts Rust and Ladybird to Qt. `BrowserView` owns one opaque Rust window-state handle and a separate `WebContentView` for each tab. It routes commands to the active tab and reports engine callbacks back to Rust. `WindowScene` attaches the active view and coordinates composition/input.
- Ladybird's existing `WebContentView` handles rendering and input. Photon compiles that frontend implementation directly from `UI/Qt`; it does not copy or fork it.

Ladybird is authoritative for the actual current URL, including redirects, links, same-document navigation, and history traversal. It is also authoritative for page titles, loading, and whether history traversal is available. Rust represents that engine-reported state for Photon; a submitted address is never treated as the final URL.

The C ABI uses one opaque `PhotonBrowserState` allocation per window. Rust passes tab identifiers and serializable snapshots only; Ladybird pointers remain in C++.

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

The Web UI uses a transparent full-window chrome surface above the active Ladybird `WebContentView`, laid out below the 72-pixel toolbar. `WindowScene` captures the toolbar and the whole page area while a React overlay is open, then forwards pointer and wheel input elsewhere to the page. React owns overlay rendering and dismissal; native code keeps only the modal input-capture flag needed to keep the underlying page inert. It also relays the page cursor to the top chrome surface while the pointer is over web content.

The bundled `load_html` chrome document is trusted. Its navigation hook consumes only validated `photon-command://` requests and cancels other top-level navigation. Hot-reload mode pins that policy to the validated `http://127.0.0.1:5173` origin. The ordinary page surface has no Photon command callback or chrome API.

Moving Ladybird rendering to a Qt Quick-native surface could remove this boundary in the future, but it is a separate integration project and is not part of the current architecture.

Each Rust tab record has a corresponding Ladybird view in the C++ adapter. Theme mode and tab metadata are Rust-owned; React keeps only temporary interaction state such as an unfinished address edit or an open popover. Native rendering and input plumbing stay in C++, while presentation and animation live in React/CSS.

## Web UI experiment

The QML frontend is now a fallback/reference path. `./photon run` creates a `WindowScene` containing two independent Ladybird web contexts:

```text
Photon window
└── WindowScene
    ├── ChromeSurface  bundled Photon React/TypeScript
    └── PageSurface    ordinary website
```

The chrome surface is full-window and transparent outside its capture regions, so its HTML/CSS popover can overlap the page while `WindowScene` forwards pointer and wheel events elsewhere to the page. The DOMs are separate. Rust still owns product state, C++ only coordinates native views, and Ladybird remains authoritative for navigation and rendering. Details and current limitations are in [WebUI.md](WebUI.md).
