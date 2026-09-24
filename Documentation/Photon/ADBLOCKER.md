# Ad blocker feature.

The photon adblocker will be a rust/c++ native add blocker built directly into the engine to prevent adverts on websites and especially youtube.

### Why not bundle UBlock Origin as an extension, similar to Helium?

We could, it is plausible, but potentially more difficult than building our own.
Reasons being, Photon uses a custom forked engine (ladybird), and not Chromium or Firefox's Gecko engine. Therefore, we cannot use chrome web extensions. So for UBlock to work, we have  to create something that will allow it to be unpacked into our browser. This could be a future project but not our priority.
