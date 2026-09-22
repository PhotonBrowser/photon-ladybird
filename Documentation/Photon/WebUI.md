# Photon React Web UI

The React/TypeScript Web UI is Photon’s default frontend. QML remains available only as a deprecated compatibility path while the migration is completed:

```text
./photon run                 # React Web UI (default)
./photon run --ui web       # explicit React Web UI
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

`WindowScene` treats toolbar and popover geometry as capture regions and forwards pointer/wheel events elsewhere to the page view. This proves capture and pass-through with separate renderers. The fixed regions are a prototype bridge; the final component API should derive hit regions declaratively from chrome DOM.

The chrome and page are separate top-level WebContent contexts. Only the bundled chrome document defines `window.photon`; ordinary pages are never injected with Photon elements or the native object. The bridge accepts only four known `photon-command:` navigation commands and exposes no filesystem, eval, arbitrary invocation, or generic JSON-RPC.

The current document is loaded through Ladybird's `load_html` internal-document path rather than a dedicated `photon://chrome/` origin. This proves renderer isolation but is a concrete remaining limitation before broader privileged APIs are added.

## State and API

Rust remains the owner of `BrowserState` and `BrowserCommand`. Ladybird remains authoritative for actual navigation, history, title, loading, and URL updates. The public web API is:

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

The Vite server is for UI-only work. The toolbar, address editor, and popover render there, but navigation commands require Photon’s native `photon-command://` handler; the dev page intentionally has no substitute bridge.

For integrated frontend development, use the native Photon window with Vite hot reload:

```bash
./photon run dev
```

This starts Vite on `127.0.0.1:5173`, waits for the project page to respond, and opens the existing Photon binary against that page. Saving a WebUI source file hot reloads the chrome without rebuilding Ladybird or Photon. The command performs a native build only when the Photon binary is missing or older than Photon’s native sources; use `./photon run --no-build dev` to require an existing up-to-date binary. Closing Photon also stops Vite. Native C++/Rust changes still require a build.

Production bundle:

```bash
cd Photon/WebUI
npm run typecheck
npm run build
```

Vite writes `dist/index.html`. `vite.config.ts` uses a relative base for embedding and `vite-plugin-singlefile` to inline the built assets. `Photon/CMakeLists.txt` tracks the WebUI source/configuration files as build dependencies, runs `npm run build` only when the output is stale, and packages `dist/index.html` as a Qt resource. `ChromeSurface` reads that built HTML and injects the initial browser state before loading it with Ladybird's internal `load_html` path. Starting an up-to-date Photon executable does not invoke Node.
