# Trusted Embedder Messaging (Implemented)

## Implemented architecture

Photon enables trusted messaging only on its chrome view. The capability is
armed for the next `load_html` call, attached to the matching committed
top-level document, and identified by a monotonically increasing generation.
The binding validates the calling document at send time. Replacing the active
document disconnects the channel and drops queued events. Ordinary views and
unrelated documents receive no binding.

```text
React / TypeScript
    ↓ window.photon
PhotonCommandTransport
    ↓ structured trusted message
Ladybird view-scoped IPC
    ↓
Photon typed decoder → PhotonApp (Rust) → AppEffects → C++ executor
```

Native state/focus events return over the same channel to TypeScript
subscribers. The public React API and Rust command/effect model are unchanged.
The temporary `photon-command://` command transport has been removed; a missing
trusted binding reports an explicit runtime error.

## Original limitation (resolved)

Before implementation, Photon sent commands by navigating its trusted chrome view to
`photon-command://...`. `ChromeSurface` intercepted that top-level navigation,
decoded the URL in Photon code, and dispatched the existing typed command. The
transport was removed after the trusted channel was validated because command
delivery should not depend on navigation behavior.

Ladybird's WebUI is a useful lifecycle precedent, but it is not a suitable
endpoint to reuse directly. `WebView::WebUI::create()` accepts only a registered
dynamic `about:` host. `WebContentPage::did_finish_loading()` invokes it only
for such a URL. The UI-process-side `WebUI` instance is stored in
`WebContentClient::m_web_ui`, while the WebContent-side `PageClient` holds a
`WebUIConnection` tied to its current top-level `Document`. The latter installs
`window.ladybird`; its `Web::Internals::WebUI::send_message()` forwards to
`PageClient`, and its native-to-JavaScript path dispatches `WebUIMessage` on the
associated document. `PageClient::page_did_change_active_document_in_top_level_browsing_context()`
clears the old connection when the document changes.

That path is tied to Ladybird's registered internal pages and its
`WebContentClient`-level host ownership. Photon needs an explicitly authorized
embedder view, a channel whose native endpoint belongs to that view, and a
binding whose call site is checked at invocation time. Reusing
`window.ladybird` would retain the built-in WebUI contract and does not by
itself enforce the child-frame caller restriction.

## Current source paths

### `load_html`

`UI/Qt/WebContentView` derives from `WebView::ViewImplementation` and inherits
`load_html(StringView)`; there is no Qt-specific `load_html` implementation.
The current path is:

```text
Photon::ChromeSurface::load
  → Ladybird::WebContentView::load_html(StringView)
  → WebView::ViewImplementation::load_html(StringView)
  → WebContentClient::async_load_html(page_id, html, navigation_id)
  → WebContentServer.ipc: load_html(PageId, ByteString, Utf16String)
  → Services/WebContent/ConnectionFromClient::load_html(...)
  → WebContentPage::page().load_html(html, navigation_id)
  → Web::Page::load_html(...)
  → LocalTraversable::navigate(about:srcdoc, document_resource, ...)
  → top-level active Document creation/activation
```

The IPC definition is `Services/WebContent/WebContentServer.ipc`; its generated
endpoint methods are build outputs, not files to edit. `ConnectionFromClient`
resolves the routed `PageId` before forwarding the HTML to the page.

### Existing WebUI, both directions

```text
WebContentPage::did_finish_loading (UI process)
  → WebView::WebUI::create(client, page_id, about-host)
  → paired IPC transport; async_connect_to_web_ui(page_id, remote_handle)
  → WebContentServer.ipc connect_to_web_ui
  → ConnectionFromClient::connect_to_web_ui
  → WebContentPage::connect_to_web_ui
  → PageClient::connect_to_web_ui
  → WebUIConnection(document)
  → window.ladybird installed in document realm
```

JavaScript calls `window.ladybird.sendMessage(name, data)`. The generated
binding reaches `Web::Internals::WebUI::send_message()`, then
`PageClient::received_message_from_web_ui()`, `WebUIConnection`, the
`WebUIClient` IPC endpoint, and the registered `WebView::WebUI` interface
callback. In the reverse direction the UI host sends `WebUIServer::send_message`;
`WebUIConnection::send_message()` JSON-deserializes the payload in the stored
document's browsing context and dispatches `WebUIMessage` on that document.

The binding is installed against the relevant realm of the active top-level
document supplied to `PageClient::connect_to_web_ui`. `PageClient` owns the
WebContent-side connection and clears it when
`page_did_change_active_document_in_top_level_browsing_context` reports a
different document. The UI side currently stores a single `RefPtr<WebUI>` on
`WebContentClient`; it is not the per-embedder-view endpoint Photon needs.

## Implemented capability and API

Ladybird's generic trusted embedder messaging facility transports JSON values
and knows nothing about Photon commands, tabs, or windows.

The capability is an explicit, one-shot authorization attached to one
`ViewImplementation` and one generated navigation ID:

```cpp
view.enable_trusted_embedder_messaging(on_message_callback);
view.load_html(chrome_html);
```

Public types in `Libraries/LibWebView/TrustedEmbedderMessaging.h`:

```cpp
struct TrustedEmbedderMessage {
    String type;
    JsonValue payload;
};

using TrustedEmbedderMessageCallback = Function<void(TrustedEmbedderMessage)>;

struct TrustedEmbedderMessagingLimits {
    size_t maximum_serialized_message_size { 64 * 1024 };
    size_t maximum_type_size { 128 };
};
```

The exact native-facing API on `ViewImplementation` is:

```cpp
ErrorOr<void> enable_trusted_embedder_messaging(
    TrustedEmbedderMessageCallback,
    TrustedEmbedderMessagingLimits = {});
ErrorOr<void> send_trusted_embedder_message(TrustedEmbedderMessage const&);
void disable_trusted_embedder_messaging();
```

`enable...` arms exactly the next top-level `load_html()` call; it does not
grant privilege to generic `load()`, a later navigation, or another view.
Calling it twice before the load is an error. If the armed load fails or is
superseded, the capability is discarded. A default view has no channel. The
view owns the callback and limits, and invokes the callback on its UI/event-loop
thread. The callback must not capture an unowned view pointer. Disable, view
destruction, document replacement, and endpoint disconnect clear the callback
and active generation. Sending while disabled or disconnected returns an
error; it never queues an event for a future document.

`ViewImplementation` owns the host-side IPC endpoint and callback. The paired
endpoint is scoped to its `PageId` and fresh channel generation. It must not
hold a raw `WebContentView*`; the view owns its page association and closes the
endpoint before clearing callbacks on replacement/destruction.

The channel clamps incoming and outgoing messages to 64 KiB and message types
to 128 bytes. Types must be non-empty UTF-8 strings and payloads must be JSON
values. Invalid or oversized messages are rejected before the Photon callback;
Photon still applies its command allowlist and validates command-specific
values. The native-to-document queue holds at most 64 messages and 1 MiB of
serialized data, dropping oldest events on overflow and clearing the queue when
the authorized document is revoked.

## IPC messages

The implementation uses a distinct paired transport per authorized document, not the existing
shared `WebUI` connection. Its endpoint definitions are
`Services/WebContent/TrustedEmbedderMessagingClient.ipc` and
`Services/WebContent/TrustedEmbedderMessagingServer.ipc`:

```text
endpoint TrustedEmbedderMessagingClient {
    received_message(u64 generation, String type, JsonValue payload) =|
}

endpoint TrustedEmbedderMessagingServer {
    send_message(u64 generation, String type, JsonValue payload) =|
}
```

This follows Ladybird's client/server endpoint convention. The UI-process
channel implements the client endpoint and sends events through the server
endpoint. The WebContent document connection implements the server endpoint
and sends its messages through the client endpoint. The paired transport
already binds the endpoints to one authorized page connection; each endpoint
also carries a monotonically increasing per-view generation on every message
to reject stale queued work after revocation.

The routed operation in `Services/WebContent/WebContentServer.ipc` is:

```text
connect_trusted_embedder_messaging(
    PageId page_id,
    Utf16String navigation_id,
    u64 generation,
    IPC::TransportHandle handle) =|
```

It is a UI-process-to-WebContent operation. The UI side sends it only after
`WebContentPage::did_finish_loading` has matched the armed `load_html`
navigation ID and confirmed the committed top-level document. The IPC handle
is therefore the authorization; ordinary HTML loading does not include a
capability flag or handle. The WebContent side accepts it only for the routed
top-level page's current active `Document`, rejects duplicate/stale
generations, and exposes no page-originated operation for enabling a channel.
Revoke is local on document replacement; closing the paired transport is
authoritative.

This avoids changing generic `load_html` IPC or adding a pre-load authorization
round trip. `PageClient` records the completed top-level navigation ID before
the UI process connects the endpoint; the WebContent side accepts the channel
only when that ID still identifies the current active document. If a
renderer/process swap means the target document is hosted elsewhere, no grant
is forwarded automatically; the host must explicitly reauthorize the new
view/document.

## Document lifecycle

The capability is a tuple of `(PageId, navigation_id, generation,
Document identity)`. It is not an origin grant and does not survive by URL.

1. Native code arms the next `load_html()` on one view. `ViewImplementation`
   records that request and its callback. The existing `load_html` IPC remains
   unchanged and generates the navigation ID.
2. `WebContentPage::did_finish_loading` matches that exact navigation ID and
   creates a fresh paired transport. It sends the handle and generation to
   `PageClient`, which connects the handle to the current active top-level
   `Document` and installs the binding. No child-document creation path
   installs the binding.
3. A same-document navigation retains the same `Document` and therefore the
   same channel. It does not mint a new grant.
4. A full navigation, history traversal to a different document, replacement
   `load_html`, reload that creates a new `Document`, page close, or process
   replacement revokes the old connection before the new document can use it.
   The new document receives no binding unless the embedder explicitly arms a
   new trusted load.
5. If history restores the exact same live `Document`, the capability remains
   only while that document remains the active authorized document. A
   different restored document has no capability.
6. View destruction closes the endpoint and clears the callback. Renderer
   recreation does not reconstitute a grant from URL, history state, or origin.

Channel connection is activated from the matching
`WebContentPage::did_finish_loading` transaction, after navigation identity
has been checked. Do not install it by observing a URL such as `about:srcdoc`;
that URL is not proof of embedder trust.

Photon's `ChromeSurface` also filters top-level navigation requests. Ladybird
implements `WebContentView::load_html()` by starting an internal
`about:srcdoc` navigation. The chrome filter must allow that one request
initiated by its pending native `load_html()` call, or the intended document
is canceled before the matching completion and channel-connection hooks run.
This exception only permits the engine's load operation: messaging remains
authorized by the generated navigation ID and the committed `Document`, never
by the `about:srcdoc` URL. The one-shot allowance is cleared by the first
navigation request and rejects it unless its URL is `about:srcdoc`.

## Frame security

The binding is installed only into the authorized top-level document's realm,
but that alone is insufficient: same-origin child code may read
`parent.<binding>` and call a function obtained there.

The binding checks the caller at invocation time. The generated native methods
in `Web::Internals::TrustedEmbedderMessaging` compare the caller's incumbent
settings object's realm/document to the connection's authorized document
using Ladybird's `HTML::incumbent_settings_object()`/callback-incumbent
machinery, rather than relying on the binding object's realm or `this` value.
The checks require all of:

* the incumbent settings object has the exact authorized `Document`;
* that document is still the active document of the local top-level
  traversable;
* its connection generation is current and not revoked.

Calls from a child frame are rejected even when the child is same-origin and
invokes the method through `parent` or `top`; a caller-context mismatch raises
a WebIDL `SecurityError`. Stale or revoked channels do not deliver messages.

Native-to-JavaScript event payloads are queued on the authorized document's
connection. Ladybird dispatches a data-free, non-bubbling,
non-composed availability event on that document. The top-level binding's
`receiveMessages()` method drains the queue only after the same incumbent
document and active-document checks used for `postMessage()`. This avoids
exposing event payloads to same-origin child code listening on
`parent.document`. The queue is bounded by message count and aggregate bytes;
old messages are dropped on overflow and all remaining messages are discarded
with the document connection. A child may observe the availability signal but
cannot read the queued payload.

## Binding and payload format

`Libraries/LibWeb/Internals/TrustedEmbedderMessaging.idl`, `.h`, and `.cpp`
define the binding:

```webidl
[LegacyNoInterfaceObject]
interface TrustedEmbedderMessaging {
    undefined postMessage(DOMString type, any payload);
    DOMString receiveMessages();
};
```

The operations post and receive structured messages; there is no method lookup,
evaluation, native object access, or reply callback. `payload` is cloned using
Ladybird's WebDriver JSON clone path, matching the existing WebUI facility.
The native endpoint carries a message type plus `JsonValue` payload. The
binding rejects non-JSON values, serialization failures, invalid strings, and
payloads exceeding the configured serialized-size limit before IPC send. The
receiving side checks the limit again after serialization/decoding.

Expose the object under a dedicated internal name, for example
`window.embedderMessaging`, only after authorization. The name is an
implementation detail, not a generic `window.native.invoke` surface. Native
code treats `type` as untrusted and dispatches only through its own explicit
allowlist.

Native event payloads remain structured `JsonValue` values in the document
connection. A data-free `TrustedEmbedderMessageAvailable` DOM event wakes
Photon's existing transport, which calls `receiveMessages()` to pull the
queued `{ type, payload }` records and convert them into the existing typed
`PhotonTransportEvent` subscription API. Queue contents are never transferred
to a later document: if the authorized document or channel is gone, they are
dropped. A new document gets a fresh channel only after explicit authorization.

## Implemented file set

Ladybird changes are represented by the consolidated Photon patch
`Patches/ladybird/0010-add-trusted-embedder-messaging-channel.patch`:

| Files | Responsibility |
| --- | --- |
| `Libraries/LibWebView/TrustedEmbedderMessaging.h/.cpp` | Public typed message/callback/limits API and view-scoped IPC channel. |
| `Libraries/LibWebView/ViewImplementation.h/.cpp` | Per-view opt-in, next-`load_html` authorization, navigation identity and generation, stale grant cancellation, native send/disable. |
| `Libraries/LibWebView/WebContentPage.cpp`, `Services/WebContent/ConnectionFromClient.cpp`, `WebContentServer.ipc` | Route the explicitly authorized page and generation to WebContent. |
| `Services/WebContent/TrustedEmbedderMessagingClient.ipc`, `TrustedEmbedderMessagingServer.ipc`, `TrustedEmbedderMessagingConnection.h/.cpp` | Bidirectional structured IPC, bounded event queue, document binding installation and revocation. |
| `Services/WebContent/PageClient.h/.cpp`, `Libraries/LibWeb/Page/Page.h` | Bind to the active top-level document, validate document/generation on send/receive, clear the connection on replacement. |
| `Libraries/LibWeb/Internals/TrustedEmbedderMessaging.idl/.h/.cpp`, `Forward.h`, `idl_files.cmake`, `Libraries/LibWeb/CMakeLists.txt`, `Libraries/LibWebView/CMakeLists.txt`, `Services/WebContent/CMakeLists.txt` | Narrow JavaScript binding and generated-source/build registration. |

`UI/Qt/WebContentView` needs no Ladybird modification: it inherits the
`ViewImplementation` API. Photon-owned integration is in
`Photon/Bridge/ChromeSurface.cpp/.h`,
`Photon/Bridge/PhotonCommandTransport.cpp/.h`, and
`Photon/WebUI/src/bridge/transport.ts`. ChromeSurface opts in before `load_html`
and supplies the typed native callback. The decoder accepts only bounded JSON
messages with the explicit Photon command allowlist. React and Rust command,
state, and effect types did not change for transport removal.

## Photon integration

Production integration is:

```text
ChromeSurface creates its existing trusted WebContentView
  → arms one trusted embedder channel on that view
  → loads bundled chrome with load_html
  → view/document receives the bound internal messaging object
  → React PhotonCommandTransport posts { type, payload }
  → Ladybird paired channel
  → Photon native typed decoder and allowlist
  → existing PhotonCommand dispatch
  → PhotonApp (Rust)
```

Reverse events use the same channel:

```text
Rust snapshot / Photon native event
  → ChromeSurface sends structured event through the view channel
  → Ladybird dispatches on the authorized document
  → PhotonCommandTransport.subscribe listener
  → existing React state/event handlers
```

The React public API, `PhotonCommand`, Rust `PhotonApp`, `AppCommand`,
`AppEffects`, and native application dispatch stay unchanged. Only the
transport implementation and its native typed decoder change. The development
Vite view is not authorized by this `load_html`-scoped capability; its
privileged commands fail closed with a missing-channel diagnostic. Loopback
origin and query parameters do not grant the capability.

## Security properties and threat model

| Threat | Ladybird guarantee | Photon responsibility / residual risk |
| --- | --- | --- |
| Malicious ordinary webpage | No opt-in means no binding or endpoint. | Ensure only the separate chrome view arms it. |
| Malicious child frame | No binding installed in child; incumbent caller-document check rejects invocation through `parent`/`top`. | Keep chrome content and CSP narrow; do not treat same-origin as authorization. |
| Same-origin child frame | Same caller check; top-level object possession is insufficient. | Keep the direct `top` invocation rejection covered in runtime security checks. |
| Navigation away/full replacement | Old document connection is revoked on active-document change; authorization is not copied to the new document. | ChromeSurface may still cancel unexpected top-level chrome navigation. |
| Same-document navigation | Same document, same channel; no new capability is granted. | Validate commands as usual. |
| Compromised React/frontend | It can send arbitrary bytes within the bounded channel. | Native decoder validates type, fields, lengths, enums, IDs, and command allowlist; React compromise is not a trust boundary. |
| Malformed or oversized message | Binding and IPC decoder reject malformed/over-limit data; invalid JSON values never reach host callback. | Validate every Photon command argument and fail closed. |
| Stale document/channel reference | Generation plus exact document identity and active-document checks reject sends; closed endpoint drops events. | Do not retain channel handles across chrome reload/recreation. |
| Renderer/WebContent restart/process swap | Old paired endpoint closes; no automatic grant follows a new process/document. | Explicitly rearm only for the intended trusted chrome document. |

The Ladybird layer provides scoped transport and lifecycle checks, not Photon
authorization semantics. Photon owns the command allowlist and all business
validation. There is no generic native invocation or `eval` facility.

## Alternatives rejected

* **Continue navigation interception:** command delivery would remain a fake
  navigation, depend on URL parsing, and could affect navigation state or
  behavior. The generic navigation callback remains available for legitimate
  navigation policy, but it is no longer a Photon command transport.
* **Inject a JavaScript function from Photon:** executing host-authored JS is
  not a native message endpoint; it offers no structured IPC, caller-realm
  authorization, or safe lifecycle by itself.
* **Reuse `window.ladybird` directly:** it is an internal WebUI binding tied to
  dynamic `about:` pages and lacks the required embedder-view opt-in and
  invocation-time child-frame check.
* **Authorize by origin:** `about:srcdoc` and embedder-loaded HTML do not provide
  a useful stable origin boundary, same-origin children inherit origin, and
  origin does not identify an authorized view/document instance.

## Reuse versus a new facility

Reusing the WebUI endpoint definitions and JSON conventions would save some
plumbing, but retaining the `WebUI` class would couple authorization to
registered `about:` hosts and the current client-global host ownership. The
proposed small generic facility reuses Ladybird's paired IPC transports,
`JsonValue`, WebDriver JSON cloning, document activation callback, and custom
event machinery while giving the embedder explicit per-view/per-document
capability semantics. It is a modest, coherent upstreamable API for embedders
that own trusted UI documents; Ladybird receives only generic message types
and payloads, never Photon concepts.

Existing WebUI could later share lower-level JSON serialization/event helpers,
but should not share its authorization/host-registration layer until both
facilities demonstrably have the same lifecycle contract.

## Validation status

The generic facility is implemented in consolidated patch 0010; Photon enables
it only for its chrome view. On the freshly materialized Release build after
removing the URL fallback, a real `window.photon.tabs.create()` call reached
the native decoder as `new-tab`, then one native state event reached the
TypeScript subscriber. Earlier runtime security checks confirmed an ordinary
page has no binding, a same-origin iframe call through `top` is rejected, and
replacing the trusted document removes the binding. Payload and event-queue
bounds are enforced in the implementation. A full interactive command sweep
was not completed in the headless test environment.

Automated coverage should continue to target caller-document identity,
replacement/reload lifecycle, malformed and oversized input, queue overflow,
and stale generations. The implementation does not grant authorization from
an origin or URL and does not automatically reauthorize a replacement page.

The API is generic enough to plausibly upstream: it provides an opt-in
structured message channel for one embedder-owned view/document, while the
engine handles transport and lifecycle and the embedder retains message
meaning.
