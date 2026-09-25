# Building Photon

From the repository root:

```bash
./photon doctor
./photon build
./photon run # builds the Vite production bundle, no dev server needed
./photon run --dev # or `./photon run dev`: Vite hot reload for the React chrome
```

`./photon` delegates configuration and compilation to `Meta/ladybird.py`, using the Qt frontend and the normal Ladybird vcpkg/build environment. Builds run from `Build/Source`, a link to a generated Git worktree containing the recorded Ladybird revision and the enabled Photon patch series. If `.photon/worktree` exists, build, run, and test use its linked edit worktree instead. The canonical Ladybird files stay pristine. Actual worktrees live beside the checkout so Cargo sees the correct workspace. Each worktree builds in its own `Build/` directory so Qt-generated relative paths remain valid. `./photon engine clean` parks build directories in the sibling `.<checkout>-photon-build/` directory when it removes generated source.

Before building or running, the CLI verifies the recorded upstream base and creates or validates the generated engine tree. It refuses to use a stale or edited generated build tree. Run `./photon sync` to merge upstream and verify the enabled patch series against the new base. Use `./photon patches enable ID` and `./photon patches disable ID` to toggle individual patches; the CLI resets and reapplies patches in existing generated trees after checking for uncaptured edits, preserving build directories so Ninja can reuse unaffected outputs. See [Patches.md](Patches.md) for the engine edit, patch capture, and toggle workflow.

Use `./photon run --no-build` to launch an existing binary. `./photon clean` removes the Photon executable and Photon-specific generated CMake output for the selected preset; it preserves vcpkg and shared Cargo artifacts. `./photon engine clean` removes generated engine source trees while preserving build artifacts.

The default preset is `Release`. Use `./photon build --debug` for a Debug build.

The React Web UI is the default frontend. `./photon build` and `./photon run` install Web UI dependencies when `Photon/WebUI/node_modules` is missing or stale, then run the Vite production build when `Photon/WebUI/dist/index.html` is missing or older than the WebUI sources. CMake keeps the same `npm run build` step as a fallback for direct Ninja builds, then packages that static HTML resource into Photon. Node is a build-time dependency and is not started when launching an up-to-date binary. For standalone development and explicit bundle checks, see [WebUI.md](WebUI.md). The React WebUI is the only Photon frontend.

Use `./photon run --dev` (or the `./photon run dev` shorthand) for live development. Dev mode installs dependencies when needed, starts Vite on `127.0.0.1:5173`, and runs Photon against that server instead of the production bundle, so it never runs `npm run build`. Frontend edits hot reload through Vite; C++, Rust, and build configuration edits rebuild Photon and restart both Photon and Vite.
