# Building Photon

From the repository root:

```bash
./photon doctor
./photon build
./photon run # builds the Vite production bundle, no dev server needed
./photon run --dev # or `./photon run dev`: Vite hot reload for the React chrome
```

`./photon` delegates configuration and compilation to `Meta/ladybird.py`, using the Qt frontend and the normal Ladybird vcpkg/build environment. The first Ladybird dependency bootstrap can be expensive. Later builds target only `photon` and are incremental.

Before building or running, the CLI verifies and materializes `Patches/series.toml` over the recorded upstream base as needed. It refuses to build when committed Ladybird files differ from the recorded base or when patches are partial or divergent. Run `./photon sync` to merge upstream, record the verified base, and materialize its patch series. See [Patches.md](Patches.md) for the patch checks and sync behavior.

Use `./photon run --no-build` to launch an existing binary. `./photon clean` removes the Photon executable and Photon-specific generated CMake output for the selected preset; it preserves vcpkg and shared Cargo artifacts.

The default preset is `Release`. Use `./photon build --debug` for a Debug build.

The React Web UI is the default frontend. `./photon build` and `./photon run` install Web UI dependencies when `Photon/WebUI/node_modules` is missing or stale, then run the Vite production build when `Photon/WebUI/dist/index.html` is missing or older than the WebUI sources. CMake keeps the same `npm run build` step as a fallback for direct Ninja builds, then packages that static HTML resource into Photon. Node is a build-time dependency and is not started when launching an up-to-date binary. For standalone development and explicit bundle checks, see [WebUI.md](WebUI.md). The React WebUI is the only Photon frontend.

Use `./photon run --dev` (or the `./photon run dev` shorthand) for live development. Dev mode installs dependencies when needed, starts Vite on `127.0.0.1:5173`, and runs Photon against that server instead of the production bundle, so it never runs `npm run build`. Frontend edits hot reload through Vite; C++, Rust, and build configuration edits rebuild Photon and restart both Photon and Vite.
