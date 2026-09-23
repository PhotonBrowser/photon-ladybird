# Photon patch policy

A Photon patch is a required change to a Ladybird-owned implementation file. New code under `Photon/`, Photon documentation, metadata, and the developer CLI are ordinary Photon files and are not patches.

The ordered manifest is `Patches/series.toml`; patch files live in `Patches/ladybird`. Each manifest entry gives the patch ID, file, purpose, and affected area.

```bash
./photon patches status
./photon patches check
```

`status` distinguishes applied patches from unapplied or divergent patches, then checks for direct Ladybird edits. `status`, `check`, and build materialization also compare committed Ladybird paths since the recorded base, so a committed engine edit cannot bypass the patch series. When the series is applied, it compares every patch-owned file byte-for-byte with a temporary export of the recorded upstream revision plus the complete series. `check` always verifies that the ordered series applies to that pristine revision and, when materialized, checks that the working files exactly match the result. It also fails on unrepresented modified or untracked Ladybird paths.

`./photon build` and `./photon run` materialize the series before invoking Ladybird's build. They apply patches only when every patch is unapplied and the full series passes `git apply --check`; afterwards they compare the patch-owned files with the recorded base plus series. A partially applied or divergent series stops with an error; the tooling never resets or overwrites files. `./photon sync` accepts a clean worktree with either no materialized patches or the exact registered patch materialization and no staged changes. Before a merge it reverses only that verified materialization, checks the series against the fetched target, merges, verifies the full ordered series, records the base, and rematerializes the patches in one command. If a patch no longer applies, sync leaves the merged Ladybird checkout pristine and reports the refresh step. After editing only `Patches/`, rerun `./photon sync`; it verifies those are the only local changes and completes the base update/materialization without requiring a temporary commit. Photon source changes still require a clean worktree. Git merge conflicts stop for manual resolution.

When an upstream change is unavoidable:

1. Confirm composition from `Photon/` cannot provide the result.
2. Make the smallest focused Ladybird change.
3. Export only that Ladybird-owned diff to a numbered patch.
4. Add its metadata to `series.toml`.
5. Run `./photon patches check` and document the reason in the change review.

The series contains the build integration patch, generic transparent canvas/view behavior, WebContent IPC and Qt forwarding for transparent composition, a generic top-level navigation interception callback, and CSS corner-shape painting/serialization support. Photon uses the navigation callback to consume its `photon-command://` transport without navigating away from the chrome document. Photon-owned browser composition and policy remain in `Photon/` and `Photon/Rust/`.
