# Photon patch workflow

The canonical Ladybird files in the repository always remain at the revision recorded in `Meta/Photon/upstream.toml`. Ladybird changes are stored as numbered patches in `Patches/ladybird`, ordered by `Patches/series.toml`.

```text
recorded Ladybird revision + ordered Photon patch series
                         │
                         ├── Build/Source       link to generated build worktree
                         └── .photon/worktree   link to editable engine worktree
```

The links point to Git worktrees in the sibling `.<checkout>-photon-worktrees/` directory, based on the recorded upstream revision. Keeping their actual paths outside the canonical checkout prevents Cargo from discovering the wrong Rust workspace. Each worktree builds in its own real `Build/` directory, which keeps CMake and Qt-generated relative paths valid. `./photon engine clean` moves those build directories into the sibling `.<checkout>-photon-build/` directory before removing generated source; the next materialization restores them to the matching worktree. If `.photon/worktree` exists, build, run, and test use it so engine edits can be built before capture; otherwise they use `Build/Source`.

## Commands

```bash
./photon engine edit
./photon patches capture "Implement prefetch" --area Networking
./photon patches status
./photon patches check
./photon engine materialize
./photon engine clean
```

`engine edit` creates `.photon/worktree` with the upstream source and registered patches. Edit Ladybird files there. `patches capture` compares that tree with a temporary export of the recorded upstream revision plus the current series. It writes the delta as the next numbered patch, appends its metadata to `series.toml`, and leaves the edited engine tree in place with the new patch represented. It refuses staged edits and validates that the new patch applies after the current series. Photon-owned files are never included.

Build, run, and test create `Build/Source` as needed and apply patches there. The canonical Ladybird files therefore stay pristine and `git status` shows only actual Photon and patch-series changes. `engine materialize` creates the generated build tree on demand. `engine clean` removes generated source trees only when they match the registered series; it refuses to remove an edited worktree so uncaptured work is preserved. Build artifacts remain parked in the sibling build directory.

`patches status` reports the generated tree state and checks that canonical Ladybird files remain pristine. `patches check` verifies that the ordered series applies cleanly to the recorded revision and validates any existing build tree. `sync` updates the upstream revision only after validating the series against the new base. It clears exact generated trees before merging and preserves edited worktrees by stopping rather than deleting them.

When an upstream change is unavoidable:

1. Confirm composition from `Photon/` cannot provide the result.
2. Create an engine edit tree with `./photon engine edit`.
3. Make the smallest generic Ladybird change.
4. Capture it with `./photon patches capture "description" --area Area`.
5. Run `./photon patches check` and build the result.

Never put Photon product policy or names in Ladybird patches. The series contains generic build integration, transparent composition, navigation interception, corner-shape painting, WebGL fallback, color-scheme refresh, traversable cleanup, and pointer-capture behavior.
