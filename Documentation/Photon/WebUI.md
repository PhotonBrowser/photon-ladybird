# Photon React Web UI

The React/TypeScript Web UI is Photon’s default frontend. QML remains available only as a deprecated compatibility path while the migration is completed:

```text
./photon run                 # React Web UI (default, production bundle, no dev server)
./photon run --ui web       # explicit React Web UI
./photon run --dev          # Vite dev server with hot reload (shorthand: ./photon run dev)
./photon run --ui qml       # deprecated QML fallback
```

React and TypeScript use a conventional Vite project. Vite and Node are build-time tools only; Photon loads the static `Photon/WebUI/dist/index.html` resource at runtime. The production build inlines its JavaScript and CSS into that HTML file so the native internal-document loader does not need an HTTP server or a Node process.

## Composition

```text
Photon window
└── WindowScene
    ├── ChromeSurface   bundled Photon React/TypeScript
    └── PageSurface     ordinary URL / normal WebContent context
```

The two independent Ladybird `WebContentView` instances use Ladybird's existing per-view backing-store and presentation path. `ChromeSurface` is full-window and painted above `PageSurface`; its transparent HTML background lets the page remain visible. The popover is a React component rendered as ordinary HTML/CSS with opacity and transform transitions. It is not a QML popup and does not share the page DOM.

The checked-out compositor does not expose a generic multi-surface native scene, so this prototype keeps the scene coordinator in Photon and makes no Ladybird engine/compositor changes. A later generic surface API should replace this coordinator without adding Photon policy to Ladybird.

## Input and security

`WindowScene` captures the 72-pixel titlebar and toolbar, active internal pages, and the full scene while a React overlay is open. It forwards pointer and wheel events elsewhere to the active page view. The overlay scrim owns outside clicks; native code only blocks page input until React closes the overlay. Qt focus selects whether keyboard events go to the chrome or the active page. The tab state lives in Rust; each tab keeps a separate Ladybird `WebContentView` so engine history survives tab switches.

The chrome and page are separate top-level WebContent contexts. Only the bundled chrome document defines `window.photon`; ordinary pages are never injected with Photon elements or the native object. The bridge accepts a fixed set of navigation, tab, theme, and overlay-capture commands. Ladybird’s registered navigation-request hook consumes `photon-command://` requests before they replace the chrome document. C++ validates the command host, query shape, and each argument before dispatch. It also cancels every other top-level navigation from the bundled chrome document, preventing a link or redirect from moving the privileged surface to a website. Hot-reload mode permits only the pinned `http://127.0.0.1:5173` origin. Rust owns browser state and URL normalization; the bridge exposes no filesystem access, arbitrary invocation, or generic JSON-RPC.

The bundled document is loaded through Ladybird's `load_html` internal-document path rather than a dedicated `photon://chrome/` origin. The native navigation policy keeps that surface on the bundled document, but a dedicated internal origin would provide a clearer engine-level identity if Photon later adds broader privileged APIs.

## State and API

Rust owns tab order, active-tab identity, internal page routes, theme mode, and each tab's presentation state. Ladybird remains authoritative for web-page URL, history, title, loading, and navigation capability updates. The public web API includes navigation, tab, theme, and browser-state subscriptions.

```ts
interface PhotonApi {
    navigation: {
        navigate(url: string): void;
        back(): void;
        forward(): void;
        reload(): void;
    };
    browser: { state: BrowserState };
}
```

## Build

Standalone UI development:

```bash
cd Photon/WebUI
npm install
npm run dev
```

The Vite server is for UI-only work. The browser chrome renders with a sample new-tab state, but navigation and tab commands require Photon’s native `photon-command://` handler; standalone development does not install a fake native bridge.

For integrated frontend development, use the native Photon window with Vite hot reload:

```bash
./photon run --dev
# shorthand: ./photon run dev
```

This starts Vite on `127.0.0.1:5173`, waits for the project page to respond, and opens the existing Photon binary against that page. Saving a WebUI source file hot reloads the chrome without rebuilding Ladybird or Photon. The command performs a native build only when the Photon binary is missing or older than Photon’s native sources; use `./photon run --no-build --dev` to require an existing up-to-date binary. Closing Photon also stops Vite. Native C++/Rust changes still require a build. Dev mode never runs the production bundle; plain `./photon run` builds `dist/index.html` so the chrome renders without a dev server.

Production bundle:

```bash
cd Photon/WebUI
npm run typecheck
npm run build
```

The interface follows the Electron Photon browser chrome and uses HeroUI, Tailwind CSS, and Lucide icons. The stylesheet defines semantic color, radius, and spacing tokens and exposes them to Tailwind utilities; both themes resolve through the same semantic roles. Vite writes `dist/index.html`; `vite.config.ts` uses a relative base and `vite-plugin-singlefile` to inline the built assets. `./photon build` and `./photon run` install dependencies when needed and run `npm run build` when the bundle is stale; `Photon/CMakeLists.txt` keeps the same `npm run build` step as a fallback for direct Ninja builds, then packages `dist/index.html` as a Qt resource. `ChromeSurface` loads that HTML and injects the initial Rust browser snapshot. Native tab, engine, and preference updates return to React through a narrow `photon-state` event. Starting an up-to-date Photon executable does not invoke Node.

The toolbar always contains one omnibox. It displays the active web page URL and stays empty on internal pages such as `photon://newtab` and `photon://settings`. The New Tab page uses the Electron Photon logo-and-wordmark layout and relies on this toolbar field for search rather than rendering a duplicate input. Tab icons use Ladybird's favicon callback with the Photon monochrome mark for internal pages. The Settings page follows Electron's grouped settings panel with Photon-supported Appearance and Privacy sections. Theme selection updates the React chrome and all current and newly created Ladybird page views (`prefers-color-scheme`). The source brand kit is in `Photon/Brand/brand-kit.zip`; the color and monochrome SVG logo masters are in `Photon/WebUI/src/assets/`.

Browser keyboard shortcuts are registered on the native Photon window, so they work while either WebContent view has focus. Ctrl/Command+L focuses the omnibox; Ctrl/Command+T, W, and R create, close, and reload tabs; F5 reloads; Alt+Left/Right navigates history; Ctrl+Tab and Ctrl+Shift+Tab (also Ctrl+PageDown/PageUp) cycle tabs. Rust determines the next tab from its tab order. The only shortcut notification sent to React is a fixed `photon-focus-address` event for focusing the omnibox; React does not register browser-wide key handlers.

While a HeroUI overlay is open, the chrome owns pointer and wheel input across the scene. `WindowScene` also intercepts events Qt delivers directly to the page child and sends them to the chrome view, keeping the page inert. A React scrim dims the full composition and receives outside clicks; the popover stays above the scrim. Clicking outside closes the overlay and restores normal page pass-through. Qt focus changes switch Ladybird's active WebContent view between chrome and page; internal pages restore focus to the chrome surface.
