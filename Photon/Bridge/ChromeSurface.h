/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#pragma once

#include <AK/Function.h>

#include <LibURL/URL.h>

#include <Photon/Bridge/PhotonCommand.h>
#include <Photon/Bridge/PhotonCommandTransport.h>

#include <QObject>
#include <QPoint>
#include <QUrl>

class QWidget;

namespace Ladybird {

class WebContentView;

}

namespace Photon {

class BrowserView;

class ChromeSurface final : public QObject {
    Q_OBJECT
public:
    explicit ChromeSurface(QWidget& parent);
    virtual ~ChromeSurface() override;

    Ladybird::WebContentView& view() { return *m_view; }
    void load(BrowserView const& browser);
    void update_state(BrowserView const& browser);
    void update_page_tooltip(QString const& text, QPoint position);
    void clear_page_tooltip();
    void focus_address_bar();
    void blur_address_bar();

    Function<void(PhotonCommand const&)> on_command;

private:
    bool handle_navigation_request(URL::URL const&);

    Ladybird::WebContentView* m_view { nullptr };
    QUrl m_dev_server_origin;
    PhotonCommandTransport m_command_transport;
    bool m_trusted_document_loaded { false };
    bool m_trusted_load_html_navigation_pending { false };
};

}
