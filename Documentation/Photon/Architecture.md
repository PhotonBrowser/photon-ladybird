# Photon architecture

Photon is an overlay on Ladybird. Ladybird remains responsible for the web platform, rendering, networking, media, IPC, sandboxing, and helper processes.

```text
User command

QML toolbar
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
QML property binding
```

## Responsibilities

- `Photon/UI` owns visual composition and address-field editing state. It declares the navigation toolbar and the rectangle occupied by web content; it forwards commands without parsing URLs or maintaining history.
- `Photon/Rust` owns the product-facing state for the attached surface: current URL, title, loading state, and back/forward availability. It normalises conservative hostname input and produces typed navigate, reload, back, and forward commands. It does not duplicate Ladybird's history.
- `Photon/Bridge` adapts Rust and Ladybird to Qt. `BrowserView` owns one opaque Rust state handle and wraps one `WebContentView`. It consumes typed commands synchronously, subscribes to Ladybird callbacks and navigation actions, updates Rust, then emits narrow Qt property notifications.
- Ladybird's existing `WebContentView` handles rendering and input. Photon compiles that frontend implementation directly from `UI/Qt`; it does not copy or fork it.

Ladybird is authoritative for the actual current URL, including redirects, links, same-document navigation, and history traversal. It is also authoritative for page titles, loading, and whether history traversal is available. Rust represents that engine-reported state for Photon; a submitted address is never treated as the final URL.

The C ABI uses one opaque `PhotonBrowserState` allocation per view. Borrowed strings are copied by C++ before another Rust mutation, commands are consumed synchronously, and the adapter destroys the Ladybird view before freeing callback state. No Ladybird pointer crosses into Rust.

The current Ladybird Qt surface is a `QWidget`/`QRhiWidget`, not a `QQuickItem`. A small QWidget host and `QQuickWidget` are therefore required at the integration edge. QML still owns the visible chrome and surface geometry; the host contains no product UI. Replacing this host becomes appropriate only if Ladybird gains a supported Qt Quick-native surface.

```text
PhotonWindow
├── QQuickWidget
│   └── Photon QML chrome
├── Ladybird WebContentView
│   └── QWidget / QRhiWidget surface
└── transient Qt Quick windows
    └── UI that must overlap web content
```

The two native surfaces establish a real compositing boundary: QML `z` ordering applies only inside the `QQuickWidget` scene and cannot place an in-scene popup above the sibling Ladybird widget. Photon popups that must overlap web content therefore use Qt Quick's window-backed popup path (`Popup.Window`). Qt gives these popup windows the correct transient relationship, positioning, focus, dismissal, and parent-window lifetime. The address field's standard Qt text-editing menu is the first use of this rule; future suggestions, menus, prompts, and popovers must use the same path when they cross the web-surface boundary. This is Photon window integration policy, not Rust state or `BrowserView` responsibility.

Moving Ladybird rendering to a Qt Quick-native surface could remove this boundary in the future, but it is a separate integration project and is not part of the current architecture.

One view and one browser state are created today. A future tab model should associate one state and browser-view instance with each tab, while tabs, workspaces, preferences, and session state belong in Rust. Native dialogs and rendering/input plumbing remain in the C++ adapter. Presentation and animation remain in QML.
