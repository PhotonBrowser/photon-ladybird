# Photon Web UI prototype

The Web UI is an explicit alternative to the frozen QML frontend:

```text
./photon run --ui web
./photon run --ui qml
```

TypeScript is bundled at build time with esbuild and loaded as static HTML, CSS, and JavaScript at runtime. Photon does not require Node to launch. React is deferred until a compatibility probe justifies it.

## Composition

```text
Photon window
└── WindowScene
    ├── ChromeSurface   bundled Photon HTML/CSS/TypeScript
    └── PageSurface     ordinary URL / normal WebContent context
```

The two independent Ladybird `WebContentView` instances use Ladybird's existing per-view backing-store and presentation path. `ChromeSurface` is full-window and painted above `PageSurface`; its transparent HTML background lets the page remain visible. The popover is ordinary HTML/CSS with opacity and transform transitions. It is not a QML popup and does not share the page DOM.

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

```bash
cd Photon/WebUI
npm install
npm run typecheck
npm run build
cd ../..
./photon build
```

The generated bundle is packaged into Photon. Runtime startup does not start Node.
