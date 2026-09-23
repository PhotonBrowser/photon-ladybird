/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#include <AK/Function.h>

#include <LibURL/URL.h>

#include <QObject>
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
    void focus_address_bar();

    Function<void(QString const&, QString const&)> on_command;

private:
    bool is_allowed_command(QUrl const&, QString&, QString&) const;
    bool handle_navigation_request(URL::URL const&);

    Ladybird::WebContentView* m_view { nullptr };
    QUrl m_dev_server_origin;
    bool m_trusted_document_loaded { false };
};

}
