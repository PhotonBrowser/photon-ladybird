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

`Meta/Photon/upstream.toml` is the single machine-readable record of the Ladybird revision on which the Photon patch series is based.

Inspect the relationship safely:

```bash
git fetch upstream
./photon upstream status
./photon patches check
```

Sync with upstream through the CLI:

```bash
./photon sync                # complete the sync and prepare the checkout
./photon sync --fetch-only    # preview divergence; change nothing
```

`sync` refuses to run with Photon source changes, staged changes, or a detached HEAD. It accepts the exact registered Ladybird patch materialization and temporarily removes it for the merge. It verifies the full patch series, merges upstream, records the verified base, and rematerializes the patches in one command. Sync never rewrites history, auto-resolves conflicts, or pushes.

If a patch no longer applies, sync merges upstream but leaves the series unapplied and the recorded base unchanged. Update the affected patch files and `Patches/series.toml`, then run the same `./photon sync` again. That second run accepts only unstaged changes under `Patches/`, verifies Ladybird files are pristine at the merged upstream, checks the entire ordered series, records the base, and materializes it. No temporary patch-refresh commit is needed. If Git reports merge conflicts, resolve them manually first; sync will not decide how an engine change should be reconciled.

Photon tooling does not reset branches, discard work, resolve conflicts, rewrite history, or push. For patch conflicts, refresh only the patch representation and rerun sync; for Git merge conflicts, resolve them manually and verify with `./photon patches check`.
