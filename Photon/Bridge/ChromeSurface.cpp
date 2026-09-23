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
#include <QRegularExpression>
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
    auto initial_state_script = QByteArrayLiteral("<script>window.__photonInitialState=") + initial_state + QByteArrayLiteral(";window.__photonPlatform='") + platform + QByteArrayLiteral("';</script>");
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
    QUrl parsed(qstring_from_ak_string(url.serialize()));
    if (!parsed.isValid())
        return true;

    if (parsed.scheme() != QStringLiteral("photon-command")) {
        if (!m_dev_server_origin.isEmpty()
            && parsed.scheme() == m_dev_server_origin.scheme()
            && parsed.host() == m_dev_server_origin.host()
            && parsed.port() == m_dev_server_origin.port())
            return false;
        return true;
    }

    QString command;
    QString value;
    if (m_trusted_document_loaded && is_allowed_command(parsed, command, value) && on_command)
        on_command(command, value);
    return true;
}

bool ChromeSurface::is_allowed_command(QUrl const& url, QString& command, QString& value) const
{
    if (!url.userInfo().isEmpty() || url.port(-1) != -1 || !url.path().isEmpty() || url.hasFragment())
        return false;

    auto const items = QUrlQuery(url).queryItems(QUrl::FullyDecoded);
    command = url.host();
    if (command == QStringLiteral("back") || command == QStringLiteral("forward")
        || command == QStringLiteral("reload") || command == QStringLiteral("new-tab")
        || command == QStringLiteral("open-settings") || command == QStringLiteral("window-minimize")
        || command == QStringLiteral("window-toggle-maximize") || command == QStringLiteral("window-close")
        || command == QStringLiteral("window-drag"))
        return !url.hasQuery() && items.isEmpty();

    if (items.size() != 1 || items.first().first != QStringLiteral("value"))
        return false;
    value = items.first().second;

    if (command == QStringLiteral("navigate"))
        return !value.trimmed().isEmpty();

    auto valid_tab_id = [](QString const& tab_id) {
        static QRegularExpression const pattern(QStringLiteral("^tab-[1-9][0-9]*$"));
        if (!pattern.match(tab_id).hasMatch())
            return false;
        bool converted = false;
        auto id = tab_id.mid(4).toULongLong(&converted);
        return converted && id != 0;
    };
    if (command == QStringLiteral("select-tab") || command == QStringLiteral("close-tab"))
        return valid_tab_id(value);

    if (command == QStringLiteral("reorder-tabs")) {
        auto const tab_ids = value.split(QLatin1Char(','), Qt::KeepEmptyParts);
        for (auto const& tab_id : tab_ids) {
            if (!valid_tab_id(tab_id))
                return false;
        }
        return !tab_ids.isEmpty();
    }

    if (command == QStringLiteral("set-theme"))
        return value == QStringLiteral("system") || value == QStringLiteral("light") || value == QStringLiteral("dark");

    if (command == QStringLiteral("set-dim-overlays"))
        return value == QStringLiteral("true") || value == QStringLiteral("false");

    if (command == QStringLiteral("capture"))
        return value == QStringLiteral("browser-menu:open") || value == QStringLiteral("browser-menu:close")
            || value == QStringLiteral("site-info:open") || value == QStringLiteral("site-info:close");

    return false;
}

}
