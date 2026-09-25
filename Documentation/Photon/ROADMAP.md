# Planned

- Build Photon DevTools
- Build Photon onboarding
- Add a prefetching system similar to Chromium Prefetch 2
- Continue profiling and optimizing Ladybird performance
- Improve browser chrome animations and transitions
- Add downloads UI and management
- Add history and bookmarks
- Add permissions and site controls
- Build Windows support (refer to ./platforms/WINDOWS.md)
- Build macOS support
- Create a reliable release and packaging system
- Expand automated testing for Photon-specific engine changes
- Implement a lightweight highly customisable native rust and/or c++ based adblocker for websites, and especially youtube, directly into the browser engine. (read ADBLOCKER.md)
- animated favicons for websites for some reason

# In progress

- Rust application and browser integration
- Website and waitlis
- Engine performance investigation and optimization


# In Review
- WebGL and rendering experiments

# Implemented

- React and TypeScript browser chrome setup
- Photon repository and project structure
- Ladybird-based engine foundation
- Add browser settings and preference
- Rust Photon application layer
- React and TypeScript browser chrome foundation
- Add proper tab management features
- Embedded HTML browser chrome
- Minimal navigation toolbar
- Back navigation
- Forward navigation
- Reload
- Editable address bar
- URL normalization
- Browser URL state propagation
- Page title propagation
- Loading state propagation
- Back and forward capability propagation
- Native window title updates
- Address editing preserved while focused
- Navigation tooltips
- Address bar context menus
- Native transient popup windows
- Basic Photon browser window and web content composition
- Bidirectional communication between browser chrome and native code
- Trusted view-scoped embedder messaging between the Photon UI and native host
- Document-scoped authorization and native event delivery
- Removed the navigation-based `photon-command://` command transport
- Individual patch enable/disable controls in the Photon patch CLI
- Bundled Inter variable fonts for the Photon WebUI
- Photon command decoding
- BrowserView lifecycle cleanup
- Fixed shutdown database assertion
- CSS `corner-shape` engine support
- Screenshot test for `corner-shape`
- Ordered Ladybird patch series
- `Patches/series.toml`
- Recorded Ladybird upstream revision
- Photon-owned code separated from Ladybird patches
- Patch creation and application tooling
- Upstream synchronization workflow
- Photon GPL-3.0 licensing separated from Ladybird's BSD-2-Clause license
- Trusted embedder messaging architecture documented
- Photon landing page foundation
- Photon branding foundation
