# Photon bridge audit

This document describes the current Photon bridge and records the retired command-URL transport for migration history. Photon chrome and ordinary web content remain separate Ladybird `WebContentView` instances.

## Current command path

```text
React components
    ↓
window.photon typed API
    ↓
PhotonCommandTransport
    ↓
trusted view-scoped embedder messaging
    ↓ Ladybird IPC
ChromeSurface → typed PhotonCommand decoder
    ↓
PhotonWindow dispatcher
    ├── PhotonApp (Rust) → AppEffects → BrowserView/Ladybird executor
    ├── WindowScene / Qt input capture
    └── Qt window operations
```

The TypeScript transport sends a structured message type and JSON payload. `PhotonCommandTransport.cpp` validates the message against Photon’s command allowlist, checks each command’s arguments, and constructs the existing typed native command. `PhotonWindow` routes application commands to Rust and Qt-specific operations to the native integration code. If trusted messaging is unavailable, dispatch fails with a diagnostic; there is no alternate command transport.

## Reverse events

```text
Ladybird observation / Rust snapshot / native focus action
    ↓
ChromeSurface sends a structured native event
    ↓ trusted embedder messaging + Ladybird IPC
PhotonCommandTransport subscribers
    ↓
React state and event handlers
```

State and focus-address events use the trusted channel. Page-tooltip notifications remain fixed native-to-chrome DOM events because they are presentation updates rather than application commands. TypeScript adapts native events to the existing subscription API; React components do not depend on Ladybird IPC details.

## Capability and security boundary

The native channel is explicitly enabled for the trusted chrome view only. Ladybird associates the grant with the intended committed top-level document, validates the caller document at invocation time, and revokes the channel and queued events on document replacement. A child frame cannot use a method obtained from `parent` or `top` to send a privileged message. Ordinary page views receive no binding. The trusted chrome is kept separate from page content; a trusted chrome navigation or replacement cannot transfer its capability to the replacement document.

Ladybird transports bounded structured JSON and has no Photon command names. Photon owns the command allowlist and validates message types and parameters. The bridge exposes no arbitrary native invocation, `eval`, filesystem operation, or generic method lookup. See [Trusted Embedder Messaging](Trusted-Embedder-Messaging.md) for the Ladybird API, lifecycle, limits, and threat model.

## Application ownership

Rust `PhotonApp` owns tab identity/order, active-tab policy, internal routes, navigation intent, preferences, and the snapshot consumed by React. Ladybird remains authoritative for actual web URL, title, loading, and history observations. C++ owns `TabId → WebContentView` associations, Ladybird callbacks and calls, view lifetimes, image conversion, Qt window operations, and execution of Rust-produced effects. The handwritten C ABI remains a small boundary for application dispatch, page observations, favicon metadata, and snapshot queries. See [Application Architecture](Application-Architecture.md) for details.

## Historical command transport

Before trusted embedder messaging, `PhotonCommandTransport` encoded commands as `photon-command://` URLs and `ChromeSurface` intercepted those top-level navigation requests. This made command delivery depend on navigation parsing and cancellation. It was removed after the trusted channel was exercised at runtime. The old scheme is not present in the current runtime command path; the historical description is retained here only to explain the migration.

The public `window.photon` API, TypeScript `PhotonCommand` model, native typed decoder, Rust `PhotonApp`, `AppCommand`, `AppEffects`, and native effect executor did not need to change when the transport changed.

## Runtime evidence and scope

Runtime checks recorded for the migration confirmed a real `new-tab` command reached the native decoder through the trusted channel, one native state event reached a TypeScript subscriber, ordinary page content had no binding, a same-origin child-frame call through `top` was rejected, and replacing the trusted document removed the binding. These checks establish representative transport and isolation behavior; they are not a claim that every browser command and every lifecycle edge was exhaustively exercised in one run.
