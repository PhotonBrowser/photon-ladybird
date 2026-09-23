# Trusted Embedder Messaging Design

## Current limitation

Photon currently sends commands by navigating its trusted chrome view to
`photon-command://...`. `ChromeSurface` intercepts that top-level navigation,
decodes the URL in Photon code, and dispatches the existing typed command. This
works, but it makes command delivery depend on navigation behavior.

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

## Proposed capability and API

Add a small generic `WebView::TrustedEmbedderMessaging` facility. It transports
JSON values and knows nothing about Photon commands, tabs, or windows.

The capability is an explicit, one-shot authorization attached to one
`ViewImplementation` and one generated navigation ID:

```cpp
view.enable_trusted_embedder_messaging(
    on_message_callback,
    maximum_message_size);
view.load_html(chrome_html);
```

Proposed public types in `Libraries/LibWebView/TrustedEmbedderMessaging.h`:

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

Suggested limits are a 64 KiB serialized incoming message and a 64 KiB
serialized outgoing event. These are channel defaults, clamped to a documented
implementation maximum. `type` is a non-empty UTF-8 string of at most 128
bytes; each payload must be a JSON value. Invalid JSON, invalid UTF-8, unknown
envelope fields if the wire decoder is strict, and over-limit messages are
dropped and logged at a rate-limited level. A bad message must not terminate
the browser process. Photon still applies its command allowlist and validates
all command-specific values.

## IPC messages

Use a distinct paired transport per authorized document, not the existing
shared `WebUI` connection. Its generated endpoint definitions should be added
as `Services/WebContent/TrustedEmbedderMessagingClient.ipc` and
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

Add one routed operation to `Services/WebContent/WebContentServer.ipc`:

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
round trip. The binding is installed after commit, so the transport must buffer
commands until it receives its ready event. If a renderer/process swap means
the target document is hosted elsewhere, no grant is forwarded automatically;
the host must explicitly reauthorize the new view/document.

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

Channel connection should be activated from the matching
`WebContentPage::did_finish_loading` transaction, after navigation identity
has been checked. Do not install it by observing a URL such as `about:srcdoc`;
that URL is not proof of embedder trust.

## Frame security

The binding is installed only into the authorized top-level document's realm,
but that alone is insufficient: same-origin child code may read
`parent.<binding>` and call a function obtained there.

The binding method must check the caller at invocation time. Implement the
generated native method in the new `Web::Internals::TrustedEmbedderMessaging`
class and compare the caller's incumbent settings object's realm/document to
the connection's authorized document. The check must use Ladybird's
`HTML::incumbent_settings_object()`/callback-incumbent machinery, not only the
binding object's realm or `this` value. Require all of:

* the incumbent settings object has the exact authorized `Document`;
* that document is still the active document of the local top-level
  traversable;
* its connection generation is current and not revoked.

Reject calls from a child frame even when the child is same-origin and invokes
the method through `parent` or `top`. A WebIDL `SecurityError` is appropriate
for a caller-context mismatch; stale/revoked channels should reject without
native delivery. Add a targeted test where the iframe obtains the parent
method reference and invokes it. Also test a child calling
`parent.<binding>.send(...)` directly.

Native-to-JavaScript events are dispatched on the authorized document, not
`window.top` and not a frame-selected target. A child may observe a bubbling
event if the API chooses bubbling, so events should be non-bubbling and
non-composed by default. Photon should use a dedicated event type with a
validated `{ type, payload }` detail. Event observation is not itself a native
capability; the caller check protects commands.

## Binding and payload format

Add `Libraries/LibWeb/Internals/TrustedEmbedderMessaging.idl`, `.h`, and `.cpp`
with one method, conceptually:

```webidl
[LegacyNoInterfaceObject]
interface TrustedEmbedderMessaging {
    undefined postMessage(DOMString type, any payload);
};
```

The only operation is posting a structured message; it has no method lookup,
evaluation, native object access, or reply callback. `payload` is cloned using
Ladybird's WebDriver JSON clone path, matching the existing WebUI facility.
The native endpoint carries a message type plus `JsonValue` payload. The
binding must reject non-JSON values, serialization failure, invalid strings,
and payloads exceeding the configured serialized-size limit before IPC send.
The receiving side checks the limit again after serialization/decoding.

Expose the object under a dedicated internal name, for example
`window.embedderMessaging`, only after authorization. The name is an
implementation detail, not a generic `window.native.invoke` surface. Native
code treats `type` as untrusted and dispatches only through its own explicit
allowlist.

Native events are delivered as a non-bubbling `EmbedderMessage` custom event
whose detail is `{ type, payload }`. Photon’s existing transport converts that
event into its current typed `PhotonTransportEvent` subscription API. Event
queues are not retained for a later document: if the authorized document or
channel is gone, the event is dropped. A new document gets a fresh ready
notification and channel only after explicit authorization.

## File-level patch plan

### Ladybird files

| File | Minimal intended change |
| --- | --- |
| `Libraries/LibWebView/TrustedEmbedderMessaging.h` (new) | Public message, callback, and payload-limit value types. |
| `Libraries/LibWebView/ViewImplementation.h` | Add explicit one-next-`load_html` enable, outbound send, and disable methods; keep disabled by default. |
| `Libraries/LibWebView/ViewImplementation.cpp` | Bind opt-in to the generated navigation ID, own callback/endpoint/generation, route sends, revoke on replacement/close. |
| `Libraries/LibWebView/WebContentPage.h` | Store pending/active grant and host endpoint scoped to this page/navigation. |
| `Libraries/LibWebView/WebContentPage.cpp` | Consume only matching authorization; create paired channel after matching top-level document activation; revoke on navigation/document replacement. |
| `Services/WebContent/WebContentServer.ipc` | Add routed channel-connect operation keyed by page, confirmed navigation ID, and generation. |
| `Services/WebContent/TrustedEmbedderMessagingClient.ipc` (new) | WebContent-to-UI structured message endpoint. |
| `Services/WebContent/TrustedEmbedderMessagingServer.ipc` (new) | UI-to-WebContent structured event endpoint. |
| `Services/WebContent/ConnectionFromClient.h/.cpp` | Validate page and pending authorization; route authorization/revocation to `PageClient`. |
| `Services/WebContent/PageClient.h/.cpp` | Own document connection; install only on authorized top-level document; clear it on active-document changes and page teardown. |
| `Services/WebContent/TrustedEmbedderMessagingConnection.h/.cpp` (new) | Install/revoke JS object, check invocation caller, clone and bound JSON, deliver/drop native events. |
| `Libraries/LibWeb/Internals/TrustedEmbedderMessaging.idl/.h/.cpp` (new) | Narrow `postMessage(type, payload)` binding and incumbent-document guard. |
| `Libraries/LibWeb/CMakeLists.txt` or current IDL source manifest | Register the new binding sources if required by this checkout's generated binding rules. |
| `Documentation/Photon/Trusted-Embedder-Messaging.md` | This design; implementation should update lifecycle/security notes if details change. |

`UI/Qt/WebContentView.h/.cpp` should need no engine change because it already
inherits `ViewImplementation`. Photon calls the new inherited API through its
existing `Ladybird::WebContentView` instance. Generated IPC and binding output
must not be committed as handwritten source unless the repository explicitly
tracks it.

### Photon files in the later implementation pass

| File | Intended change |
| --- | --- |
| `Photon/Bridge/ChromeSurface.cpp/.h` | Arm trusted messaging on the chrome view before its production `load_html`; register typed callback; send state/tooltip/focus events via the channel; remove command navigation interception. |
| `Photon/Bridge/PhotonCommandTransport.cpp/.h` | Replace URL decoding with strict JSON/message envelope decoding into existing `PhotonCommand` variants. |
| `Photon/WebUI/src/bridge/transport.ts` | Replace the navigation implementation with `window.embedderMessaging.postMessage` and event subscription; preserve `PhotonCommandTransport`, `PhotonCommand`, and React-facing methods. |
| `Photon/WebUI/src/main.tsx` | Change only transport construction/readiness hookup as needed; keep the public `window.photon` API and components unchanged. |
| `Photon/CMakeLists.txt` | Remove `PhotonCommandTransport` source only if its implementation is folded elsewhere; keep the application dispatcher unchanged. |
| Photon architecture/bridge docs | Replace scheme-specific transport documentation. |

The photon decoder should accept one JSON object shape, e.g. `{type, payload}`;
each command schema remains explicit and bounded. No Rust command/effect/state
types change.

## Photon integration

Production integration should be:

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
Vite view must be treated deliberately: do not grant a capability based only
on `127.0.0.1` or query parameters. Either provide a native-authorized trusted
document load mechanism for development, or leave native messaging disabled
in that mode.

## Security properties and threat model

| Threat | Ladybird guarantee | Photon responsibility / residual risk |
| --- | --- | --- |
| Malicious ordinary webpage | No opt-in means no binding or endpoint. | Ensure only the separate chrome view arms it. |
| Malicious child frame | No binding installed in child; incumbent caller-document check rejects invocation through `parent`/`top`. | Keep chrome content and CSP narrow; do not treat same-origin as authorization. |
| Same-origin child frame | Same caller check; top-level object possession is insufficient. | Add explicit parent-method-reference invocation tests. |
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

* **Continue navigation interception:** command delivery remains a fake
  navigation, depends on URL parsing, and can affect navigation state/behavior.
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

## Implementation sequence and tests

1. Add the generic channel message types and paired IPC endpoint definitions;
   build generated IPC and endpoint classes without changing Photon.
2. Add the explicit one-next-`load_html` opt-in, generation, and exact
   navigation-ID authorization. Test that ordinary `load_html` and all normal
   views create no endpoint.
3. Add `TrustedEmbedderMessagingConnection` and the binding. Test top-level
   delivery, serialization failures, payload bounds, and caller rejection.
4. Wire commit/revoke behavior. Test same-document navigation, full navigation,
   reload, history traversal, replacement `load_html`, close, and process
   replacement. Confirm stale event sends are dropped.
5. Add WebContent/browser tests for ordinary top-level pages, same-origin
   iframe calls through `parent` and `top`, cross-origin iframes, iframe
   document replacement, and no inherited child binding.
6. Integrate Photon transport and strict native decoding while preserving the
   existing public API and Rust application path. Test all command variants
   and reverse typed events.
7. Remove `photon-command://` interception only after command/event and security
   smoke tests pass; retain unrelated navigation interception.

The API is plausibly upstreamable because it is a generic, opt-in structured
message channel for one embedder-owned view/document, and the engine handles
transport and lifecycle while the embedder retains message meaning.
