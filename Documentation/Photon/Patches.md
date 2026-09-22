# Photon patch policy

A Photon patch is a required change to a Ladybird-owned implementation file. New code under `Photon/`, Photon documentation, metadata, and the developer CLI are ordinary Photon files and are not patches.

The ordered manifest is `Patches/series.toml`; patch files live in `Patches/ladybird`. Each manifest entry gives the patch ID, file, purpose, and affected area.

```bash
./photon patches status
./photon patches check
```

`status` distinguishes applied patches from unapplied or divergent patches, then checks for direct Ladybird edits. When the series is applied, it compares every patch-owned file byte-for-byte with a temporary export of the recorded upstream revision plus the complete series. `check` always verifies that the ordered series applies to that pristine revision and, when materialized, checks that the working files exactly match the result. It also fails on unrepresented modified or untracked Ladybird paths.

`./photon build` and `./photon run` materialize the series before invoking Ladybird's build. They apply patches only when every patch is unapplied and the full series passes `git apply --check`. A partially applied or divergent series stops with an error; the tooling never resets or overwrites files. Upstream sync requires a clean worktree and checks the complete patch series against the fetched target before merging. A patch conflict stops before the merge; merge conflicts also stop without aborting or discarding the merge.

When an upstream change is unavoidable:

1. Confirm composition from `Photon/` cannot provide the result.
2. Make the smallest focused Ladybird change.
3. Export only that Ladybird-owned diff to a numbered patch.
4. Add its metadata to `series.toml`.
5. Run `./photon patches check` and document the reason in the change review.

The series contains the build integration patch, generic transparent canvas/view behavior, and the WebContent IPC and Qt forwarding needed to use transparent composition. Photon-owned browser composition and policy remain in `Photon/`.
