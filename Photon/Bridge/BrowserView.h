/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#pragma once

#include <LibWeb/CSS/PreferredColorScheme.h>
#include <QHash>
#include <QObject>
#include <QPoint>
#include <QString>
#include <cstdint>

class QWidget;
struct PhotonBrowserCommand;
struct PhotonAppEffects;
struct PhotonBrowserState;

namespace Ladybird {

class WebContentView;

}

namespace Photon {

enum class ThemeMode;

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
    Ladybird::WebContentView& widget() const;
    uint64_t active_tab_id() const;
    QString tabs_json() const;
    bool is_internal_page() const;
    Web::CSS::PreferredColorScheme preferred_color_scheme() const;
    void refresh_preferred_color_scheme();

    Q_INVOKABLE bool navigate(QString const& input);
    Q_INVOKABLE void reload();
    Q_INVOKABLE void go_back();
    Q_INVOKABLE void go_forward();
    Q_INVOKABLE void focus_web_content();
    Q_INVOKABLE uint64_t create_tab();
    Q_INVOKABLE uint64_t open_settings();
    Q_INVOKABLE void select_tab(uint64_t tab_id);
    void select_adjacent_tab(bool previous);
    Q_INVOKABLE void close_tab(uint64_t tab_id);
    Q_INVOKABLE void reorder_tabs(QList<uint64_t> const& tab_ids);
    void set_theme_mode(ThemeMode mode);
    void set_dim_overlays(bool enabled);
    void load_initial_url();

signals:
    void url_changed();
    void title_changed();
    void loading_changed();
    void navigation_capabilities_changed();
    void cursor_changed();
    void page_tooltip_changed(QString text, QPoint position);
    void page_tooltip_cleared();
    void browser_state_changed();
    void active_tab_changed();

private:
    bool apply_command(PhotonBrowserCommand const&);
    Ladybird::WebContentView& create_view(uint64_t tab_id);
    void update_url(uint64_t tab_id, QString const&);
    void update_title(uint64_t tab_id, QString const&);
    void update_loading(uint64_t tab_id, bool);
    void update_navigation_capabilities(uint64_t tab_id);
    void emit_active_tab_state_changed();
    void sync_view_visibility();
    void apply_app_effects(PhotonAppEffects const&);

    QWidget& m_host;
    QHash<uint64_t, Ladybird::WebContentView*> m_views;
    // Opaque Rust state owns all tab identity and browser-facing state.
    PhotonBrowserState* m_state { nullptr };
};

}
