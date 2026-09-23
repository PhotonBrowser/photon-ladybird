# Photon patches

This directory records the small changes Photon must make to Ladybird-owned implementation files. The ordered manifest is `series.toml`. The CLI applies this series only to generated engine worktrees beside the checkout; `Build/Source` and `.photon/worktree` are convenient links to them. The canonical Ladybird checkout stays at the recorded pristine upstream revision.

Photon-owned files are normal source files and never belong in this series. Use `./photon engine edit` and `./photon patches capture` to turn Ladybird edits into the next numbered patch. See `Documentation/Photon/Patches.md` for the complete workflow.
