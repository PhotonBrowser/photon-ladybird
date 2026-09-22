# Photon agent guide

This repository is Photon, a custom browser frontend built on Ladybird. Use this file as the operating contract for automated agents and human contributors working on Photon.

## Core rules

- Preserve the separation between Photon product code and Ladybird engine code.
- Keep browser/application state in Rust. React may own only ephemeral presentation state such as an open popover, focus, an unfinished address edit, or animation state.
- Keep native C++ small and integration-focused. C++ may coordinate Qt widgets, WebContent views, lifecycle, input forwarding, and narrow native callbacks; it must not become the tab, settings, session, workspace, or browser-policy layer.
- Do not inject Photon UI into an ordinary webpage DOM. The chrome and webpage must remain separate rendering contexts.
- Do not expose Photon’s privileged API, DOM, state, filesystem, or native objects to ordinary websites.
- Do not add arbitrary `eval`, generic JSON-RPC, filesystem access, or unrestricted native invocation to the chrome bridge.
- Do not add new product functionality to QML. QML is a deprecated compatibility/reference frontend only.
- Prefer Photon-owned composition code over Ladybird changes. A Ladybird change is acceptable only when a small generic rendering/composition primitive is genuinely required.
- Never copy or fork large portions of Ladybird’s compositor, renderer, or WebContent implementation.
- Preserve existing user changes. Inspect the worktree before editing and do not use destructive reset or checkout commands to discard work.

## Ownership map

| Area | Responsibility | Policy |
| --- | --- | --- |
| `Photon/WebUI` | React/TypeScript chrome, HTML, CSS, bundle entrypoint | Default frontend; Node is build-time only |
| `Photon/Rust` | `BrowserState`, `BrowserCommand`, URL normalization, product state | Source of truth for Photon-owned state |
| `Photon/Bridge` | Rust/Ladybird/Qt adapter and `WindowScene` | Thin integration layer; no product policy |
| `Photon/App` | Application startup and frontend selection | React Web UI by default; QML only when explicitly selected |
| `Photon/UI` | Existing QML frontend | Frozen fallback; deprecate gradually |
| `Patches/ladybird` | Numbered patches for Ladybird-owned changes | Keep ordered by `Patches/series.toml` |
| `Documentation/Photon` | Architecture, build, patch, upstream, and Web UI docs | Update when behavior or workflow changes |
| `Tools/PhotonCLI` | `./photon` developer workflow | Use it for build, run, sync, and patch checks |

## Architecture

Photon is an overlay on Ladybird. Ladybird owns web-platform behavior, navigation, networking, rendering, history, media, sandboxing, and helper processes. Photon owns the browser product shell and state around the attached view.

```text
Photon native window
└── WindowScene
    ├── ChromeSurface: privileged React/TypeScript WebContentView
    │   └── transparent HTML/CSS chrome and overlays
    └── PageSurface: ordinary Ladybird WebContentView
        └── untrusted website
```

The chrome surface covers the native content area and is painted above the page surface. Its transparent regions leave the page visible. The page is laid out below the toolbar so resizing moves and scales both surfaces together. HTML/CSS controls layering inside the chrome renderer; the native scene only orders the major page and chrome surfaces.

### State and command flow

```text
User action in React
    ↓
window.photon.navigation.*
    ↓ photon-command:// with a small allowlist
ChromeSurface / C++ bridge
    ↓
Photon BrowserView
    ↓
Rust BrowserCommand / Ladybird navigation API

Ladybird navigation and engine events
    ↓
Photon BrowserView callback
    ↓
Rust BrowserState
    ↓
Chrome initial state / future typed state updates
    ↓
React presentation
```

Ladybird is authoritative for the actual URL, redirects, links, same-document navigation, history traversal, title, loading state, and history capability. Never treat a submitted address as confirmed browser state.

The public chrome API is intentionally small and is not React-specific:

```ts
interface BrowserState {
    url: string;
    title: string;
    loading: boolean;
    canGoBack: boolean;
    canGoForward: boolean;
}

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

The bridge must validate commands and accept only the known navigation operations. Do not make the native protocol depend on React component names or arbitrary serialized method calls.

### Input routing

`WindowScene` owns the current prototype hit regions. Toolbar and visible popover geometry capture pointer and wheel input for the chrome; events outside those regions are translated into the page view’s coordinates and forwarded to the page. Do not use alpha alone as hit testing.

The current regions are intentionally simple. Future work should derive capture regions from trusted chrome DOM/component metadata instead of making every frontend component maintain native pixel rectangles. Any change to the routing model must verify:

- toolbar controls capture clicks and keyboard focus;
- the address form submits navigation commands;
- visible popovers capture their own input and dismiss correctly;
- transparent areas pass clicks, hover, scrolling, links, and text selection to the page;
- resizing keeps the page and chrome aligned.

## React Web UI rules

- `Photon/WebUI` is a static bundle. React and TypeScript are compiled by esbuild; Photon must never start Node at runtime.
- Keep the component tree small and use plain CSS. Do not add Next.js, SSR, a large component framework, Redux, or Tailwind without a concrete requirement.
- Keep persistent browser state out of React. React state is appropriate for transient UI state only.
- Keep the chrome document separate from the page document. Only the bundled chrome document receives `window.photon`.
- Keep the internal document-loading mechanism trusted and narrow. The current implementation uses Ladybird’s internal `load_html` path; it is not yet a dedicated `photon://chrome/` origin.
- Treat the internal-origin limitation as a security boundary to improve before adding broader privileged APIs.
- Preserve the current prototype scope: Back, Forward, Reload, an editable address field, and one test popover. Do not port the entire QML frontend prematurely.

Frontend commands:

```bash
cd Photon/WebUI
npm install
npm run typecheck
npm run build
cd ../..
```

The generated files in `Photon/WebUI/dist` are packaged as Qt resources. Review bundle changes when changing dependencies or the entrypoint.

## Ladybird patch policy

Ladybird-owned implementation files must have a numbered patch in `Patches/ladybird` and an entry in `Patches/series.toml`. Photon-owned files under `Photon/`, Photon documentation, metadata, and the CLI are ordinary repository files.

The current patch series is:

- `0001-integrate-photon-build.patch` — Photon build integration and Qt Quick dependency.
- `0002-transparent-webui-composition.patch` — generic per-view transparent-canvas support used to compose the privileged chrome above the page.

When a Ladybird change is required:

1. Confirm that the behavior cannot be implemented in `Photon/` or through existing Ladybird APIs.
2. Make the smallest generic change; never add Photon product names or policy to Ladybird.
3. Export only the Ladybird-owned diff into a new numbered patch.
4. Add the patch to `Patches/series.toml` in order.
5. Keep the checkout patched so the normal build uses the behavior. `patches status` should report the patch as applied.
6. Run `./photon patches check` against the recorded upstream base.
7. Update the relevant Photon documentation.

Do not create a patch for files owned by Photon. Do not silently refresh a patch against a different upstream revision; record the new base only after the series applies cleanly.

## Upstream synchronization

Expected remotes:

```text
origin   Photon repository
upstream Ladybird repository
```

`Meta/Photon/upstream.toml` records the exact Ladybird revision on which the patch series is based. Use the Photon CLI rather than ad-hoc upstream bookkeeping:

```bash
./photon upstream status
./photon patches status
./photon patches check
./photon sync --fetch-only
./photon sync --record
```

`./photon sync --record` requires a clean branch, fetches upstream, merges without rewriting history, verifies every patch against the new upstream target, and updates only the recorded revision after verification. It does not push, reset, discard changes, or resolve conflicts automatically.

After an upstream merge:

1. Resolve conflicts manually while preserving Photon behavior.
2. Refresh any affected patch context, keeping the patch’s semantic change narrow.
3. Run patch checks and the native/frontend validation matrix.
4. Commit the upstream metadata separately from feature work when practical.

## QML deprecation

The React Web UI is the default:

```bash
./photon run
./photon run --ui web
```

The QML frontend remains a temporary fallback:

```bash
./photon run --ui qml
```

Do not remove QML until the Web UI has equivalent required behavior and a deliberate migration decision has been made. Do not add new browser features to QML. Compatibility fixes must remain minimal, and QML launch should continue to warn that it is deprecated.

## Validation checklist

Before handing off a Photon change, run the checks relevant to the touched areas:

```bash
git status --short
git diff --check
./photon patches status
./photon patches check
./photon upstream status

cd Photon/WebUI
npm run typecheck
npm run build
cd ../..

cargo test --manifest-path Tools/PhotonCLI/Cargo.toml
cargo clippy --manifest-path Tools/PhotonCLI/Cargo.toml --all-targets -- -D warnings
ninja -C Build/release Photon WebContent -j8
./photon run --no-build
./photon run --no-build --ui qml
```

Runtime smoke tests should confirm that the default React chrome starts, the webpage remains visible beneath it, the toolbar and popover are interactive, transparent areas pass input to the webpage, and the QML fallback still starts with its deprecation warning.

Before committing, confirm that no unrelated files changed and that the worktree is clean after the intended commit.
