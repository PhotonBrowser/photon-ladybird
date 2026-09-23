# Photon agent guide

This file is the operating contract for Photon contributors and coding agents. Photon is a browser product shell built on Ladybird. Keep product policy in Photon, web-platform behavior in Ladybird, and every Ladybird change in the registered patch series.

## Architecture rules

- React and TypeScript own Photon product UI. Photon-owned native C++ is a small host, composition, lifecycle, input-routing, and integration layer; do not implement product UI with Qt Widgets.
- Rust is the source of truth for persistent Photon browser/application state and commands where those are implemented. Ladybird remains authoritative for actual page navigation, URL, title, loading, and history state. React owns presentation and transient UI state.
- Keep the trusted chrome and untrusted webpage in separate WebContent rendering contexts. Never inject Photon UI into a page document or expose `window.photon` or equivalent privileged capabilities to a page.
- Validate every native command against a narrow allowlist and validate its arguments. Do not add arbitrary navigation with retained privilege, `eval`, generic JSON-RPC, filesystem access, or unrestricted native invocation.
- The legacy QML frontend has been removed. React WebUI is the only Photon product UI.
- Node/npm tooling is build-time and development-only; Photon must not start Node to run an up-to-date browser.
- Minimize divergence from Ladybird. Prefer Photon-owned code when it can solve the problem. Never copy large parts of Ladybird's compositor, renderer, or WebContent implementation.
- Preserve existing work. Inspect `git status` and diffs before editing; never use reset/checkout commands to discard changes.

## Ownership

| Path | Owns | Rules |
| --- | --- | --- |
| `Photon/WebUI` | React + TypeScript product UI, Vite build, Biome formatting/linting | Default frontend; `dist/` is generated and ignored. |
| `Photon/Bridge` | Minimal native composition/integration, `ChromeSurface`, `WindowScene`, native/WebUI boundary | No browser product policy or persistent product state. |
| `Photon/Rust` | Photon browser/application state and commands where implemented | Source of truth for persistent Photon state; no rendering. |
| `Photon/App` | Photon startup and WebUI launch | Uses the React WebUI. |
| `Photon/LICENSE` | Photon license text | Photon source is GPL-3.0-only. Keep Ladybird's root license separate. |
| `Tools/PhotonCLI` | `./photon` build, run, dev, sync, and patch workflow | Use the CLI instead of ad-hoc patch/upstream operations. |
| `Patches/ladybird` | Authoritative representation of every Ladybird-owned modification | Numbered patches registered in order in `Patches/series.toml`. |
| `Documentation/Photon`, `Meta/Photon` | Photon docs and repository/upstream metadata | Update docs when architecture or workflow changes. |
| Ladybird-owned paths outside the Photon areas above | Upstream engine and build source | **DO NOT DIRECTLY MODIFY LADYBIRD SOURCE.** Any Photon change must exist as a registered patch. |

## Runtime architecture

```text
Photon native window
|
+-- ChromeSurface
|     `-- trusted React WebUI (WebContentView)
|
`-- PageSurface
      `-- untrusted ordinary webpage (WebContentView)
```

Both surfaces use Ladybird WebContent views, but they are separate documents and contexts. `WindowScene` places the transparent chrome view over the page view and sizes the page below the toolbar. Ladybird owns webpage rendering and web-platform behavior. Photon chrome rendering is HTML/CSS in the trusted WebUI; native code only composes and hosts the views.

| Concern | Owner and current behavior |
| --- | --- |
| Persistent browser state | `Photon/Rust`; tabs, preferences, and Photon settings are serialized into snapshots. |
| Actual webpage navigation and engine state | Ladybird; callbacks update Rust state. Do not treat submitted address text as confirmed state. |
| Transient UI and overlays | React owns the open overlay, focus, and editing state. It sends a narrow capture-open/close command for overlays that must intercept page input; native keeps only the capture bit required by its input router. |
| Navigation and privileged commands | WebUI calls `window.photon`; `ChromeSurface` intercepts `photon-command://`, validates operation and arguments, and dispatches through Photon browser state/bridge. |
| Overlay appearance | React DOM/CSS owns popovers and scrims, including layering and dimming. |
| Input routing | `WindowScene` routes pointer and wheel input to chrome or page as described below; Qt/Ladybird handles focused keyboard input. |
| Native integration | `Photon/Bridge` creates/views, connects Ladybird callbacks, owns window lifecycle and composition. |

### Security boundary

- `PageSurface` content is hostile and untrusted. It must never receive `window.photon`, browser state, filesystem access, native objects, or equivalent privileged APIs.
- `ChromeSurface` loads trusted bundled Photon application code. Its commands use the fixed allowlist and validated arguments in `ChromeSurface::is_allowed_command`.
- Arbitrary top-level navigation from the trusted chrome document is canceled. The one production command transport is `photon-command://`; development permits only the pinned `http://127.0.0.1:5173` Vite origin. Do not let a navigated website retain chrome privileges.
- The bundled chrome currently uses Ladybird's internal `load_html` path rather than a dedicated chrome origin. Keep the trusted document loading path narrow and do not broaden privileged APIs based on that internal-origin limitation.
- No generic `eval`, JSON-RPC, arbitrary native invocation, or website-facing bridge is allowed.

## WebUI and build pipeline

The production pipeline is:

```text
React + TypeScript
       |
      Vite
       |
vite-plugin-singlefile
       |
generated Photon/WebUI/dist/index.html
       |
Qt/native resource
       |
ChromeSurface
```

`Photon/CMakeLists.txt` builds the Vite bundle and packages `dist/index.html` as a Qt resource. `./photon build` and normal `./photon run` ensure dependencies and rebuild stale assets. `./photon run --dev` starts Vite at `127.0.0.1:5173` for hot reload. `./photon run --no-build` launches an existing executable. `Photon/WebUI/dist/` is generated and ignored: never commit or edit it. Node/npm are required only for build and development, not browser runtime.

Biome commands are defined in `Photon/WebUI/package.json`:

```bash
cd Photon/WebUI
npm run typecheck
npm run lint
npm run format:check
npm run build
```

`npm run format` writes formatting changes. Dependencies and build tooling belong in `package.json`/lockfile; do not add a runtime Node dependency, SSR framework, or large state-management framework without a concrete need. Only the trusted chrome document receives `window.photon`.

## Input routing and focus

The current router uses native geometry and a small capture flag; it does not synchronize DOM element rectangles with native code.

- The chrome WebContent view covers the scene. `WindowScene` considers the entire view chrome-owned on Photon internal pages, the toolbar strip (currently a native 72-pixel height) on ordinary pages, and the entire scene while a native overlay capture bit is set.
- Chrome DOM/CSS uses pointer-event rules for transparent shell areas, toolbar controls, and scrims. For pointer events landing on chrome outside native-owned regions, `WindowScene` translates coordinates and forwards mouse events to the active page view. Wheel events follow the same ownership and coordinate mapping.
- React owns whether browser-menu/site-info overlays are open and sends `capture` open/close commands so native routing can prevent page interaction and forward events to the chrome while an overlay/scrim is active. Settings theme selection remains within an internal Photon page and does not need a native capture region.
- Keyboard events go to the focused WebContent view. Native window shortcuts cover browser commands such as address focus, tab operations, reload, and history. Native code does not mirror the chrome DOM tree or forward general keyboard input between documents.
- Resizing places the chrome over the whole scene and the page below the toolbar. The toolbar height is duplicated in native layout and must stay aligned with the WebUI style. Do not add per-component native pixel rectangles; if routing evolves, keep DOM/UI semantics in React and expose only narrow composition/input metadata required by native hosting.

When changing routing, verify toolbar controls and address submission, overlay/scrim capture and dismissal, page interaction through transparent regions, page scrolling and selection, focused keyboard behavior, and alignment on resize.

## Ladybird patch invariant and workflow

**DO NOT DIRECTLY MODIFY LADYBIRD SOURCE.** This applies even to small changes and even when a source checkout is materialized for building. The authoritative representation is the patch, not the materialized source edit.

- Forbidden: an intentional Ladybird change that exists only as a direct edit with no registered patch.
- Allowed: a Ladybird checkout file changed solely because a registered patch has been materialized. That file is a build input; the registered patch remains the authoritative change.

The invariant is:

```text
recorded pristine Ladybird upstream
    + ordered registered Photon patch series
    = Ladybird source used to build Photon
```

`./photon patches check` must enforce that invariant against `Meta/Photon/upstream.toml`, including a clean application of the complete ordered series and exact equality of materialized Ladybird files. It must fail on any unrepresented direct or committed Ladybird modification. Build/run materialization must stop rather than overwrite a partial, divergent, or unrepresented Ladybird edit.

When an engine change is necessary:

```text
Photon-owned solution?
        |
       yes -> implement under Photon/
        |
       no
        v
create/update Patches/ladybird/NNNN-*.patch
        |
register in Patches/series.toml
        |
materialize with the Photon patch workflow
        |
run ./photon patches status and ./photon patches check
        |
build/test
```

Prefer the smallest generic Ladybird change; never add Photon product policy or names to engine code. Do not create patches for Photon-owned files. Never leave an intentional engine change only in the checkout or tell a future agent to patch it later. Do not silently move the recorded base. Use `./photon upstream status`, `./photon sync --fetch-only`, and `./photon sync --record` for upstream operations; sync refuses unclean or unrepresented edits and does not resolve conflicts automatically.

`Patches/series.toml` is the current patch list. Do not duplicate a hard-coded list here; document semantics and invariants instead.

## Frontend selection and validation

The React WebUI is the only frontend (`./photon run`). `./photon run --dev` enables Vite hot reload. The legacy QML frontend and its `--ui qml` option have been removed.

Run relevant checks for the touched areas, including `git diff --check`, `./photon patches status`, and `./photon patches check`. For WebUI changes use the Biome/typecheck/build commands above. For native/runtime changes use the Photon CLI and relevant Ladybird build/runtime checks. Preserve unrelated worktree changes; do not require a clean worktree as a handoff condition when unrelated user changes pre-exist.
