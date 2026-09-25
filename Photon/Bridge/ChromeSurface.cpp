/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/ChromeSurface.h>

#include <AK/Debug.h>
#include <AK/JsonValue.h>
#include <LibURL/URL.h>
#include <LibWebView/TrustedEmbedderMessaging.h>
#include <UI/Qt/StringUtils.h>
#include <UI/Qt/WebContentView.h>

#include <QFile>
#include <QUrl>
#include <QUrlQuery>

namespace Photon {

static QByteArray resource(QString const& path)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return { };
    return file.readAll();
}

ChromeSurface::ChromeSurface(QWidget& parent)
    : QObject(&parent)
    , m_view(new Ladybird::WebContentView(&parent))
{
    m_view->set_transparent_background(true);
    m_view->on_navigation_request = [this](URL::URL const& url) {
        return handle_navigation_request(url);
    };
}

ChromeSurface::~ChromeSurface()
{
    delete m_view;
    m_view = nullptr;
}

void ChromeSurface::load(BrowserView const& browser)
{
    auto initial_state = browser.tabs_json().toUtf8();

    auto dev_server = qgetenv("PHOTON_WEBUI_DEV_SERVER");
    if (!dev_server.isEmpty()) {
        QUrl url(QString::fromUtf8(dev_server));
        if (!url.isValid()
            || url.scheme() != QStringLiteral("http")
            || url.host() != QStringLiteral("127.0.0.1")
            || url.port() != 5173)
            return;
        m_dev_server_origin = url;
    }

    QByteArray html;
    if (m_dev_server_origin.isEmpty()) {
        html = resource(QStringLiteral(":/Photon/WebUI/dist/index.html"));
        if (html.isEmpty())
            return;
    }

    if (m_dev_server_origin.isEmpty()) {
        auto head_end = html.indexOf("</head>");
        if (head_end < 0)
            return;
#ifdef Q_OS_MACOS
        auto platform = QByteArrayLiteral("macos");
#else
        auto platform = QByteArrayLiteral("other");
#endif
        auto initial_state_base64 = initial_state.toBase64();
        auto initial_state_script = QByteArrayLiteral("<script>window.__photonInitialState=JSON.parse(new TextDecoder().decode(Uint8Array.from(atob('")
            + initial_state_base64
            + QByteArrayLiteral("'), c => c.charCodeAt(0))));window.__photonPlatform='") + platform + QByteArrayLiteral("';</script>");
        html.insert(head_end, initial_state_script);
    }
    Optional<URL::URL> dev_server_url;
    if (!m_dev_server_origin.isEmpty()) {
        QUrl url = m_dev_server_origin;
        QUrlQuery query(url);
        query.addQueryItem(QStringLiteral("photonInitialState"), QString::fromUtf8(initial_state));
#ifdef Q_OS_MACOS
        query.addQueryItem(QStringLiteral("photonPlatform"), QStringLiteral("macos"));
#else
        query.addQueryItem(QStringLiteral("photonPlatform"), QStringLiteral("other"));
#endif
        url.setQuery(query);
        auto parsed_url = ak_url_from_qstring(url.toString());
        if (!parsed_url.has_value())
            return;
        dev_server_url = parsed_url.release_value();
    }

    auto on_message = [this](WebView::TrustedEmbedderMessage message) {
        if (!m_trusted_document_loaded || !on_command)
            return;
        auto payload = message.payload.serialized();
        auto payload_bytes = payload.bytes();
        auto command = m_command_transport.decode_message(qstring_from_ak_string(message.type),
            QByteArray(reinterpret_cast<char const*>(payload_bytes.data()), static_cast<qsizetype>(payload_bytes.size())));
        if (command.has_value())
            on_command(*command);
    };
    auto channel_result = dev_server_url.has_value()
        ? m_view->enable_trusted_embedder_messaging_for_next_navigation(*dev_server_url, move(on_message))
        : m_view->enable_trusted_embedder_messaging(move(on_message));
    if (channel_result.is_error())
        dbgln("Unable to enable Photon trusted message channel: {}", channel_result.error());
    m_trusted_document_loaded = !channel_result.is_error();
    if (!dev_server_url.has_value()) {
        m_trusted_load_html_navigation_pending = true;
        m_view->load_html({ html.constData(), static_cast<size_t>(html.size()) });
        return;
    }

    m_trusted_load_html_navigation_pending = false;
    m_view->load(*dev_server_url);
}

void ChromeSurface::update_state(BrowserView const& browser)
{
    auto state_json = browser.tabs_json().toUtf8();
    auto parsed_state = AK::JsonValue::from_string({ state_json.constData(), static_cast<size_t>(state_json.size()) });
    if (parsed_state.is_error()) {
        dbgln("Unable to parse Photon state event: {}", parsed_state.error());
        return;
    }
    auto result = m_view->send_trusted_embedder_message({ "state"_string, parsed_state.release_value() });
    if (result.is_error())
        dbgln("Unable to send Photon state event through trusted messaging: {}", result.error());
}

void ChromeSurface::update_page_tooltip(QString const& text, QPoint position)
{
    auto encoded_text = text.toUtf8().toBase64();
    auto script = QStringLiteral("window.dispatchEvent(new CustomEvent('photon-page-tooltip', { detail: { text: new TextDecoder().decode(Uint8Array.from(atob('%1'), c => c.charCodeAt(0))), x: %2, y: %3 } }));")
                      .arg(QString::fromLatin1(encoded_text))
                      .arg(position.x())
                      .arg(position.y());
    m_view->run_javascript(ak_string_from_qstring(script));
}

void ChromeSurface::clear_page_tooltip()
{
    m_view->run_javascript(ak_string_from_qstring(QStringLiteral("window.dispatchEvent(new Event('photon-page-tooltip-clear'));")));
}

void ChromeSurface::focus_address_bar()
{
    auto result = m_view->send_trusted_embedder_message({ "focus-address"_string, AK::JsonValue { } });
    if (result.is_error())
        dbgln("Unable to send focus-address event through trusted messaging: {}", result.error());
}

bool ChromeSurface::handle_navigation_request(URL::URL const& url)
{
    // WebView::load_html() uses about:srcdoc internally. Permit exactly the
    // navigation request initiated by this native load; this does not grant
    // the document a bridge. The document capability remains tied to the
    // explicit WebView opt-in and its generated navigation identity.
    if (m_trusted_load_html_navigation_pending) {
        m_trusted_load_html_navigation_pending = false;
        if (url == URL::about_srcdoc())
            return false;
    }

    QUrl parsed(qstring_from_ak_string(url.serialize()));
    if (!parsed.isValid())
        return true;

    if (!m_dev_server_origin.isEmpty()
        && parsed.scheme() == m_dev_server_origin.scheme()
        && parsed.host() == m_dev_server_origin.host()
        && parsed.port() == m_dev_server_origin.port())
        return false;
    return true;
}

}
