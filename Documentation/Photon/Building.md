# Building Photon

From the repository root:

```bash
./photon doctor
./photon build
./photon run
./photon run --ui web
./photon run dev # Vite hot reload for the React chrome
./photon run --ui qml # deprecated compatibility fallback
```

`./photon` delegates configuration and compilation to `Meta/ladybird.py`, using the Qt frontend and the normal Ladybird vcpkg/build environment. The first Ladybird dependency bootstrap can be expensive. Later builds target only `photon` and are incremental.

Before building or running, the CLI verifies and materializes `Patches/series.toml` over the recorded upstream base as needed. Divergent or partially applied patches stop the command rather than being overwritten. See [Patches.md](Patches.md) for the patch checks and sync behavior.

Use `./photon run --no-build` to launch an existing binary. `./photon clean` removes the Photon executable and Photon-specific generated CMake/QML output for the selected preset; it preserves vcpkg and shared Cargo artifacts.

The default preset is `Release`. Use `./photon build --debug` for a Debug build.

The React Web UI is the default frontend. CMake runs the Vite production build when its source or configuration is newer than `Photon/WebUI/dist/index.html`, then packages that static HTML resource into Photon. Node is a build-time dependency and is not started when launching an up-to-date binary. For standalone development and explicit bundle checks, see [WebUI.md](WebUI.md). `--ui qml` remains as a temporary compatibility fallback while the QML implementation is deprecated.
