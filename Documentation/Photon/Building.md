# Building Photon

From the repository root:

```bash
./photon doctor
./photon build
./photon run
./photon run --ui web
./photon run --ui qml # deprecated compatibility fallback
```

`./photon` delegates configuration and compilation to `Meta/ladybird.py`, using the Qt frontend and the normal Ladybird vcpkg/build environment. The first Ladybird dependency bootstrap can be expensive. Later builds target only `photon` and are incremental.

Before building or running, the CLI verifies and materializes `Patches/series.toml` over the recorded upstream base as needed. Divergent or partially applied patches stop the command rather than being overwritten. See [Patches.md](Patches.md) for the patch checks and sync behavior.

Use `./photon run --no-build` to launch an existing binary. `./photon clean` removes the Photon executable and Photon-specific generated CMake/QML output for the selected preset; it preserves vcpkg and shared Cargo artifacts.

The default preset is `Release`. Use `./photon build --debug` for a Debug build.

The React Web UI is the default frontend. Its bundle is built with Node tooling during development only; Photon runtime startup uses packaged static resources and does not start Node. `--ui qml` remains as a temporary compatibility fallback while the QML implementation is deprecated. See [WebUI.md](WebUI.md).
