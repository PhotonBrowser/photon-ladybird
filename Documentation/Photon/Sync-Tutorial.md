# Tutorial: syncing Photon with Ladybird

This tutorial walks through updating Photon’s Ladybird base. The `./photon`
CLI manages the merge, checks the registered Ladybird patches, records the new
base, and rebuilds generated engine source. You do not need to merge upstream
or edit `Meta/Photon/upstream.toml` by hand.

## Before you start

Commit or stash your work first. Sync requires a clean checkout so it can tell
its own merge and patch changes from your work. Check with:

```bash
git status --short
```

If files are listed, commit or stash them before continuing. Also make sure you
are on your Photon branch, not in a detached-HEAD state.

## Preview the update

Fetch Ladybird and ask Photon to compare it with your current branch:

```bash
./photon sync --fetch-only
```

The preview reports the upstream revision and how many commits are incoming
and local-only. It does not merge or modify the recorded base. To inspect the
tracking details separately, run:

```bash
./photon upstream status
```

## Run the sync

When you are ready, run:

```bash
./photon sync
```

Photon fetches upstream, merges it into the current branch without rewriting
history, and checks whether the enabled patch series applies to the new base.
When everything succeeds, the CLI updates `Meta/Photon/upstream.toml` and
recreates `Build/Source` from the new Ladybird revision plus the enabled
patches. Check the result with:

```bash
./photon patches status
./photon patches check
```

Then build or run Photon as usual. Generated engine source is disposable; the
numbered patches in `Patches/ladybird` and their order in
`Patches/series.toml` are the durable representation of Photon’s Ladybird
changes.

## If a Photon patch no longer applies

The sync may merge upstream and then stop because a registered patch needs
updating. It leaves `Meta/Photon/upstream.toml` at the old base. This is
intentional: the base is recorded only after the whole enabled series applies.

Update the affected patch file or files under `Patches/ladybird`, and update
`Patches/series.toml` if the patch list or metadata needs to change. Keep the
canonical Ladybird source files untouched. Then rerun:

```bash
./photon sync
```

The retry checks that the changes are limited to the patch series, validates
the complete enabled series against the already-merged Ladybird revision,
records the new base, and recreates the generated source. You do not need a
temporary commit for the patch refresh.

## If the merge finished but the base was not recorded

Do not reset the branch or try to merge upstream again. First make sure your
own work is committed or stashed, then retry using the tracking ref already
fetched by the previous run:

```bash
./photon sync --no-fetch
```

This lets Photon recognize the completed merge, check the patch series, and
finish recording the base. If the retry says the checkout contains unrelated
changes, inspect `git status --short` and commit or stash those changes before
trying again. The sync command never discards them.

`--no-fetch` means “use the local upstream tracking ref”; it is useful after an
interrupted or partially completed sync. Run plain `./photon sync` when you
want to fetch upstream again. There is no `--record` option: successful sync
records the base automatically.

## If Git reports merge conflicts

Photon does not choose how to resolve source conflicts. Resolve the conflicts
in Git, finish the merge, and run:

```bash
./photon patches check
./photon sync --no-fetch
```

If the patches need to change, follow [the patch refresh steps](#if-a-photon-patch-no-longer-applies).
Do not update `Meta/Photon/upstream.toml` manually to bypass a failed patch
check.

## Quick reference

| Command | Use it for |
| --- | --- |
| `./photon sync --fetch-only` | Preview upstream divergence without merging. |
| `./photon sync` | Fetch, merge, validate patches, and record the new base. |
| `./photon sync --no-fetch` | Finish or retry a sync using the already-fetched upstream ref. |
| `./photon patches status` | See which patches are enabled and whether generated source exists. |
| `./photon patches check` | Confirm the enabled series applies cleanly to the recorded base. |
