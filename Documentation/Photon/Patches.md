# Photon patch policy

A Photon patch is a required change to a Ladybird-owned implementation file. New code under `Photon/`, Photon documentation, metadata, and the developer CLI are ordinary Photon files and are not patches.

The ordered manifest is `Patches/series.toml`; patch files live in `Patches/ladybird`. Each manifest entry gives the patch ID, file, purpose, and affected area.

```bash
./photon patches status
./photon patches check
```

`status` reports whether each patch appears in the working tree. `check` exports the revision from `Meta/Photon/upstream.toml` into a temporary directory and applies the complete series there with `git apply --check`. It never changes the working tree.

When an upstream change is unavoidable:

1. Confirm composition from `Photon/` cannot provide the result.
2. Make the smallest focused Ladybird change.
3. Export only that Ladybird-owned diff to a numbered patch.
4. Add its metadata to `series.toml`.
5. Run `./photon patches check` and document the reason in the change review.

The series contains the build integration patch and the transparent-composition patch. The latter is intentionally narrow: it adds a per-view transparent canvas flag so Photon’s privileged chrome can render above page content without changing the default canvas behavior of ordinary webpages.
