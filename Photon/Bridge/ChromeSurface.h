/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#include <AK/Function.h>

#include <QObject>

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

    Function<void(QString const&, QString const&)> on_command;

private:
    void handle_url_change(QString const& url);

    Ladybird::WebContentView* m_view { nullptr };
};

}
