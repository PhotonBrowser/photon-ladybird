/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/ChromeSurface.h>

#include <LibURL/URL.h>
#include <UI/Qt/StringUtils.h>
#include <UI/Qt/WebContentView.h>

#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
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
    m_view->on_url_change = [this](URL::URL const& url) {
        handle_url_change(qstring_from_ak_string(url.serialize()));
    };
}

ChromeSurface::~ChromeSurface()
{
    delete m_view;
    m_view = nullptr;
}

void ChromeSurface::load(BrowserView const& browser)
{
    auto html = resource(QStringLiteral(":/Photon/WebUI/dist/index.html"));
    if (html.isEmpty())
        return;

    auto initial_state = QJsonDocument(QJsonObject {
                                           { QStringLiteral("url"), browser.url() },
                                           { QStringLiteral("title"), browser.title() },
                                           { QStringLiteral("loading"), browser.loading() },
                                           { QStringLiteral("canGoBack"), browser.can_go_back() },
                                           { QStringLiteral("canGoForward"), browser.can_go_forward() },
                                       })
                             .toJson(QJsonDocument::Compact);
    auto head_end = html.indexOf("</head>");
    if (head_end < 0)
        return;
    auto initial_state_script = QByteArrayLiteral("<script>window.__photonInitialState=") + initial_state + QByteArrayLiteral(";</script>");
    html.insert(head_end, initial_state_script);
    auto document = QString::fromUtf8(html).toUtf8();
    m_view->load_html({ document.constData(), static_cast<size_t>(document.size()) });
}

void ChromeSurface::handle_url_change(QString const& url)
{
    QUrl parsed(url);
    if (!parsed.isValid() || parsed.scheme() != QStringLiteral("photon-command"))
        return;

    auto command = parsed.host();
    auto value = QUrlQuery(parsed).queryItemValue(QStringLiteral("value"));
    if (on_command)
        on_command(command, value);
}

}
