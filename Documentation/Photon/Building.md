# Building Photon

From the repository root:

```bash
./photon doctor
./photon build
./photon run
```

`./photon` delegates configuration and compilation to `Meta/ladybird.py`, using the Qt frontend and the normal Ladybird vcpkg/build environment. The first Ladybird dependency bootstrap can be expensive. Later builds target only `photon` and are incremental.

Use `./photon run --no-build` to launch an existing binary. `./photon clean` removes the Photon executable and Photon-specific generated CMake/QML output for the selected preset; it preserves vcpkg and shared Cargo artifacts.

The default preset is `Release`. Use `./photon build --debug` for a Debug build.
