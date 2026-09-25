# Ladybird upstream workflow

The expected remotes are:

```text
origin    https://github.com/PhotonBrowser/photon-ladybird.git (Photon repository)
upstream  https://github.com/LadybirdBrowser/ladybird.git
```

`Meta/Photon/origin.toml` is the canonical record for `origin`. Keep it in sync with:

```bash
./photon remote status
./photon remote set-origin <url-or-owner/repo>
```

`Meta/Photon/upstream.toml` is the single machine-readable record of the Ladybird revision on which the Photon patch series is based. Materialized engine worktrees contain the ordered enabled subset of the registered patches; patches are enabled by default and can be toggled individually with `./photon patches enable ID` or `./photon patches disable ID`.

Inspect the relationship safely:

```bash
git fetch upstream
./photon upstream status
./photon patches check
```

Sync with upstream through the CLI:

```bash
./photon sync                # complete the sync and validate generated engine trees
./photon sync --fetch-only    # preview divergence; change nothing
```

`sync` refuses to run with Photon source changes, staged changes, or a detached HEAD. Canonical Ladybird source is pristine; generated engine trees are checked and safely removed before a merge, then recreated from the new base and enabled patch series. Sync never rewrites history, auto-resolves conflicts, or pushes.

If a patch no longer applies, sync merges upstream but leaves the recorded base unchanged. Update the affected patch files and `Patches/series.toml`, then run the same `./photon sync` again. That second run accepts only unstaged changes under `Patches/`, verifies canonical Ladybird files are pristine at the merged upstream, checks the ordered enabled series, records the base, and recreates the generated build source. No temporary patch-refresh commit is needed. If Git reports merge conflicts, resolve them manually first; sync will not decide how an engine change should be reconciled.

Photon tooling does not reset branches, discard work, resolve conflicts, rewrite history, or push. For patch conflicts, refresh only the patch representation and rerun sync; for Git merge conflicts, resolve them manually and verify with `./photon patches check`.
