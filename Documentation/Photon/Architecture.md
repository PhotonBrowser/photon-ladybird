# Photon architecture

Photon is an overlay on Ladybird. Ladybird remains responsible for the web platform, rendering, networking, media, IPC, sandboxing, and helper processes.

```text
User command

React component
    ▼
window.photon typed API
    ▼
PhotonCommand (TypeScript)
    ▼
PhotonCommandTransport (temporary navigation adapter)
    ▼
ChromeSurface → PhotonCommandTransport decoder → Window::dispatch_command
    ├── BrowserView → typed Rust C ABI / BrowserState → Ladybird WebContentView
    ├── WindowScene → Qt input-capture state
    └── PhotonWindow → Qt window operations

Engine and application event

Ladybird callback / Qt action
    ▼
BrowserView → Rust BrowserState snapshot
    ▼
ChromeSurface → photon-state event → React subscribers
```

## Responsibilities

- `Photon/WebUI` owns browser chrome composition, address editing, popovers, and internal-page presentation. It sends typed commands and renders snapshots; it does not own browser or tab state.
- `Photon/Rust` owns tab order, active-tab identity, internal-page routes, appearance preferences, and each tab's URL, title, loading state, and history capabilities. Ladybird remains authoritative for the history of every web tab.
- `Photon/Bridge` adapts Rust and Ladybird to Qt. `BrowserView` owns one opaque Rust window-state handle and a separate `WebContentView` for each tab. It routes commands to the active tab and reports engine callbacks back to Rust. `WindowScene` attaches the active view and coordinates composition/input.
- Ladybird's existing `WebContentView` handles rendering and input. Photon compiles that frontend implementation directly from `UI/Qt`; it does not copy or fork it.

Ladybird is authoritative for the actual current URL, including redirects, links, same-document navigation, and history traversal. It is also authoritative for page titles, loading, and whether history traversal is available. Rust represents that engine-reported state for Photon; a submitted address is never treated as the final URL.

The C ABI uses one opaque `PhotonBrowserState` allocation per window. Rust passes tab identifiers and serializable snapshots only; Ladybird pointers remain in C++.

The current Ladybird Qt surface is a `QWidget`/`QRhiWidget`. Photon uses a small QWidget scene coordinator at the integration edge.

```text
PhotonWindow
└── WindowScene
    ├── ChromeSurface: trusted React WebUI
    └── PageSurface: untrusted webpage
```

The Web UI uses a transparent full-window chrome surface above the active Ladybird `WebContentView`, laid out below the 72-pixel toolbar. `WindowScene` captures the toolbar and the whole page area while a React overlay is open, then forwards pointer and wheel input elsewhere to the page. React owns overlay rendering and dismissal; native code keeps only the modal input-capture flag needed to keep the underlying page inert. It also relays the page cursor to the top chrome surface while the pointer is over web content.

The bundled `load_html` chrome document is trusted. Its top-level navigation hook passes the temporary navigation transport to a Photon-owned decoder, which returns a typed `PhotonCommand`; invalid command requests and other top-level navigation are canceled. `Window::dispatch_command` is the native routing boundary. It sends browser/tab/preferences operations through `BrowserView` to typed Rust state APIs, and sends window/input operations to Qt. Hot-reload mode permits only the pinned `http://127.0.0.1:5173` origin. The ordinary page surface has no Photon command callback or chrome API. The trusted bootstrap encodes its initial snapshot as base64 data before parsing it in JavaScript.

The TypeScript `PhotonCommandTransport` interface keeps the public `window.photon` methods independent of the current URL adapter. Replacing this adapter and the C++ `PhotonCommandTransport` decoder with a dedicated trusted-context native binding should leave React callers, `Window::dispatch_command`, Rust browser-state methods, and snapshot events unchanged. A native binding itself is not available from the current Ladybird surface; see the [bridge audit](Bridge-Audit.md) for the current API, security review, and migration boundary.

Each Rust tab record has a corresponding Ladybird view in the C++ adapter. Theme mode and tab metadata are Rust-owned; React keeps only temporary interaction state such as an unfinished address edit or an open popover. Native rendering and input plumbing stay in C++, while presentation and animation live in React/CSS.

## Runtime composition

`./photon run` creates a `WindowScene` containing two independent Ladybird web contexts:

```text
Photon window
└── WindowScene
    ├── ChromeSurface  bundled Photon React/TypeScript
    └── PageSurface    ordinary website
```

The chrome surface is full-window and transparent outside its capture regions, so its HTML/CSS popover can overlap the page while `WindowScene` forwards pointer and wheel events elsewhere to the page. The DOMs are separate. Rust still owns product state, C++ only coordinates native views, and Ladybird remains authoritative for navigation and rendering. Details and current limitations are in [WebUI.md](WebUI.md).
