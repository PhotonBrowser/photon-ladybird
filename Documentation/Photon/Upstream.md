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
./photon sync --fetch-only   # fetch and report the divergence, change nothing
./photon sync                # fetch, then merge the upstream tracking branch
./photon sync --record       # merge, verify the patch series, record the new base
```

`sync` refuses to run with a dirty working tree or a detached HEAD, never
rewrites history, never auto-resolves conflicts, and never pushes. A merge
conflict stops the command for manual resolution. `--record` rewrites only
the `revision` line in `Meta/Photon/upstream.toml`, and only after the patch
series is verified against the merged base.

To update, begin from a clean topic branch, fetch `upstream`, and rebase or merge according to the repository's policy. Resolve conflicts manually, rebuild, run the focused tests, and update the recorded revision only after the result is verified. Refresh any affected files in `Patches/ladybird` so the series applies to the new base.

Photon tooling does not reset branches, discard work, resolve conflicts, rewrite history, or push, apart from `sync`'s explicit fetch and conflict-free merge. A conflict is an explicit maintenance task.
