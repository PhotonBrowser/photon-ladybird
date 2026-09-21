/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#include <QObject>
#include <QString>

class QWidget;
struct PhotonBrowserCommand;
struct PhotonBrowserState;

namespace Ladybird {

class WebContentView;

}

namespace Photon {

class BrowserView final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString url READ url NOTIFY url_changed)
    Q_PROPERTY(QString title READ title NOTIFY title_changed)
    Q_PROPERTY(bool loading READ loading NOTIFY loading_changed)
    Q_PROPERTY(bool canGoBack READ can_go_back NOTIFY navigation_capabilities_changed)
    Q_PROPERTY(bool canGoForward READ can_go_forward NOTIFY navigation_capabilities_changed)

public:
    explicit BrowserView(QWidget& host);
    virtual ~BrowserView() override;

    QString url() const;
    QString title() const;
    bool loading() const;
    bool can_go_back() const;
    bool can_go_forward() const;
    Ladybird::WebContentView& widget() const { return *m_view; }

    Q_INVOKABLE bool navigate(QString const& input);
    Q_INVOKABLE void reload();
    Q_INVOKABLE void go_back();
    Q_INVOKABLE void go_forward();
    Q_INVOKABLE void focus_web_content();
    void load_initial_url();

signals:
    void url_changed();
    void title_changed();
    void loading_changed();
    void navigation_capabilities_changed();

private:
    bool apply_command(PhotonBrowserCommand const&);
    void update_url(QString const&);
    void update_title(QString const&);
    void update_loading(bool);
    void update_navigation_capabilities();

    // The host owns this QWidget child; the destructor deletes it before releasing callback state.
    Ladybird::WebContentView* m_view { nullptr };
    // Opaque and uniquely owned here. No Ladybird pointer crosses into Rust.
    PhotonBrowserState* m_state { nullptr };
};

}
