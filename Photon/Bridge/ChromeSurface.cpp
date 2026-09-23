/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/ChromeSurface.h>

#include <LibURL/URL.h>
#include <UI/Qt/StringUtils.h>
#include <UI/Qt/WebContentView.h>

#include <QFile>
#include <QUrl>

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
        m_trusted_document_loaded = true;
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
        m_view->load(parsed_url.release_value());
        return;
    }

    auto html = resource(QStringLiteral(":/Photon/WebUI/dist/index.html"));
    if (html.isEmpty())
        return;

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
    auto document = QString::fromUtf8(html).toUtf8();
    m_trusted_document_loaded = true;
    m_view->load_html({ document.constData(), static_cast<size_t>(document.size()) });
}

void ChromeSurface::update_state(BrowserView const& browser)
{
    auto state = browser.tabs_json().toUtf8().toBase64();
    auto script = QStringLiteral("window.dispatchEvent(new CustomEvent('photon-state', { detail: JSON.parse(new TextDecoder().decode(Uint8Array.from(atob('%1'), c => c.charCodeAt(0)))) }));")
                      .arg(QString::fromLatin1(state));
    m_view->run_javascript(ak_string_from_qstring(script));
}

void ChromeSurface::focus_address_bar()
{
    m_view->run_javascript(ak_string_from_qstring(QStringLiteral("window.dispatchEvent(new Event('photon-focus-address'));")));
}

bool ChromeSurface::handle_navigation_request(URL::URL const& url)
{
    if (m_command_transport.handles(url)) {
        if (m_trusted_document_loaded && on_command) {
            if (auto command = m_command_transport.decode(url))
                on_command(*command);
        }
        return true;
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
