/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <Photon/Bridge/BrowserView.h>

#include <AK/Base64.h>
#include <LibGfx/ImageFormats/PNGWriter.h>
#include <LibWeb/CSS/PreferredColorScheme.h>
#include <LibWebView/Menu.h>
#include <Photon/Rust/PhotonCore.h>
#include <UI/Qt/StringUtils.h>
#include <UI/Qt/WebContentView.h>

#include <QSet>
#include <QWidget>

namespace Photon {

static QString qstring_from_photon_utf8(PhotonUtf8 value)
{
    return QString::fromUtf8(reinterpret_cast<char const*>(value.data), static_cast<qsizetype>(value.len));
}

static PhotonBrowserCommand prepare_navigation(PhotonBrowserState* state, QString const& input)
{
    auto utf8 = input.toUtf8();
    return photon_browser_prepare_navigate(
        state,
        reinterpret_cast<uint8_t const*>(utf8.constData()),
        static_cast<size_t>(utf8.size()));
}

static Web::CSS::PreferredColorScheme preferred_color_scheme(uint8_t mode)
{
    switch (mode) {
    case 1:
        return Web::CSS::PreferredColorScheme::Light;
    case 2:
        return Web::CSS::PreferredColorScheme::Dark;
    default:
        return Web::CSS::PreferredColorScheme::Auto;
    }
}

class NavigationActionObserver final : public WebView::Action::Observer {
public:
    explicit NavigationActionObserver(Function<void()> state_changed)
        : m_state_changed(move(state_changed))
    {
    }

    virtual void on_enabled_state_changed(WebView::Action&) override
    {
        m_state_changed();
    }

private:
    Function<void()> m_state_changed;
};

BrowserView::BrowserView(QWidget& host)
    : QObject(&host)
    , m_host(host)
    , m_state(photon_browser_state_new())
{
    create_view(active_tab_id());
}

BrowserView::~BrowserView()
{
    for (auto* view : m_views)
        delete view;
    m_views.clear();
    photon_browser_state_free(m_state);
    m_state = nullptr;
}

QString BrowserView::url() const
{
    return qstring_from_photon_utf8(photon_browser_url(m_state));
}

QString BrowserView::title() const
{
    return qstring_from_photon_utf8(photon_browser_title(m_state));
}

bool BrowserView::loading() const
{
    return photon_browser_loading(m_state) != 0;
}

bool BrowserView::can_go_back() const
{
    return photon_browser_can_go_back(m_state) != 0;
}

bool BrowserView::can_go_forward() const
{
    return photon_browser_can_go_forward(m_state) != 0;
}

Ladybird::WebContentView& BrowserView::widget() const
{
    auto* view = m_views.value(active_tab_id());
    VERIFY(view);
    return *view;
}

uint64_t BrowserView::active_tab_id() const
{
    return photon_browser_active_tab_id(m_state);
}

QString BrowserView::tabs_json() const
{
    auto json = photon_browser_tabs_json(m_state);
    return qstring_from_photon_utf8(json);
}

bool BrowserView::is_internal_page() const
{
    return photon_browser_is_internal_page(m_state) != 0;
}

Ladybird::WebContentView& BrowserView::create_view(uint64_t tab_id)
{
    auto* view = new Ladybird::WebContentView(&m_host);
    m_views.insert(tab_id, view);
    view->set_preferred_color_scheme(preferred_color_scheme());
    view->hide();
    view->on_url_change = [this, tab_id](URL::URL const& url) {
        update_url(tab_id, qstring_from_ak_string(url.serialize()));
    };
    view->on_title_change = [this, tab_id](Utf16String const& title) {
        update_title(tab_id, qstring_from_utf16_string(title));
    };
    view->on_loading_state_change = [this, tab_id](bool loading) {
        update_loading(tab_id, loading);
    };
    view->on_favicon_change = [this, tab_id](Optional<Gfx::Bitmap const&> const& favicon) {
        auto favicon_url = QString {};
        if (favicon.has_value()) {
            auto png = Gfx::PNGWriter::encode(*favicon);
            if (png.is_error())
                return;
            auto encoded = encode_base64(png.value().bytes());
            if (encoded.is_error())
                return;
            favicon_url = QStringLiteral("data:image/png;base64,") + qstring_from_ak_string(encoded.value().bytes());
        }
        auto utf8 = favicon_url.toUtf8();
        if (photon_browser_set_favicon(m_state, tab_id, reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size())) == 0)
            return;
        emit browser_state_changed();
    };

    auto page_cursor_changed = move(view->on_cursor_change);
    view->on_cursor_change = [this, tab_id, page_cursor_changed = move(page_cursor_changed)](Gfx::Cursor const& cursor) {
        if (page_cursor_changed)
            page_cursor_changed(cursor);
        if (tab_id == active_tab_id())
            emit cursor_changed();
    };

    view->navigate_back_action().add_observer(adopt_own(*new NavigationActionObserver([this, tab_id] {
        update_navigation_capabilities(tab_id);
    })));
    view->navigate_forward_action().add_observer(adopt_own(*new NavigationActionObserver([this, tab_id] {
        update_navigation_capabilities(tab_id);
    })));
    return *view;
}

bool BrowserView::navigate(QString const& input)
{
    return apply_command(prepare_navigation(m_state, input));
}

void BrowserView::reload()
{
    apply_command(photon_browser_prepare_reload(m_state));
}

void BrowserView::go_back()
{
    apply_command(photon_browser_prepare_back(m_state));
}

void BrowserView::go_forward()
{
    apply_command(photon_browser_prepare_forward(m_state));
}

void BrowserView::focus_web_content()
{
    if (!is_internal_page())
        widget().setFocus(Qt::ShortcutFocusReason);
}

uint64_t BrowserView::create_tab()
{
    return activate_created_tab(photon_browser_create_tab(m_state));
}

uint64_t BrowserView::open_settings()
{
    return activate_created_tab(photon_browser_open_settings(m_state));
}

uint64_t BrowserView::activate_created_tab(uint64_t tab_id)
{
    if (tab_id == 0)
        return 0;
    create_view(tab_id);
    sync_view_visibility();
    emit active_tab_changed();
    emit_active_tab_state_changed();
    return tab_id;
}

void BrowserView::select_tab(uint64_t tab_id)
{
    if (!photon_browser_select_tab(m_state, tab_id))
        return;
    sync_view_visibility();
    emit active_tab_changed();
    emit_active_tab_state_changed();
}

void BrowserView::select_adjacent_tab(bool previous)
{
    select_tab(photon_browser_adjacent_tab_id(m_state, previous));
}

void BrowserView::close_tab(uint64_t tab_id)
{
    auto old_ids = QSet<uint64_t> {};
    for (size_t i = 0; i < photon_browser_tab_count(m_state); ++i)
        old_ids.insert(photon_browser_tab_id_at(m_state, i));
    auto old_active_tab_id = active_tab_id();
    if (!photon_browser_close_tab(m_state, tab_id))
        return;

    auto new_ids = QSet<uint64_t> {};
    for (size_t i = 0; i < photon_browser_tab_count(m_state); ++i)
        new_ids.insert(photon_browser_tab_id_at(m_state, i));
    auto removed_ids = old_ids - new_ids;
    auto active_changed = old_active_tab_id != active_tab_id();
    sync_view_visibility();
    if (active_changed)
        emit active_tab_changed();

    for (auto removed_id : removed_ids) {
        auto* view = m_views.take(removed_id);
        delete view;
    }
    emit_active_tab_state_changed();
}

void BrowserView::reorder_tabs(QList<uint64_t> const& tab_ids)
{
    if (!photon_browser_reorder_tabs(m_state, tab_ids.constData(), static_cast<size_t>(tab_ids.size())))
        return;
    emit browser_state_changed();
}

void BrowserView::set_theme_mode(QString const& mode)
{
    auto value = mode == QStringLiteral("light") ? 1 : mode == QStringLiteral("dark") ? 2 : 0;
    if (!photon_browser_set_theme_mode(m_state, static_cast<uint8_t>(value)))
        return;
    auto color_scheme = preferred_color_scheme(static_cast<uint8_t>(value));
    for (auto* view : m_views)
        view->set_preferred_color_scheme(color_scheme);
    emit browser_state_changed();
}

void BrowserView::load_initial_url()
{
    if (is_internal_page())
        return;
    if (!navigate(url()))
        warnln("Photon: Rust core supplied an invalid initial URL");
}

bool BrowserView::apply_command(PhotonBrowserCommand const& command)
{
    switch (command.kind) {
    case PhotonBrowserCommandKind_Navigate: {
        auto url = qstring_from_photon_utf8(command.argument);
        auto parsed_url = ak_url_from_qstring(url);
        if (!parsed_url.has_value())
            return false;
        auto utf8 = url.toUtf8();
        if (!photon_browser_begin_navigation(m_state, active_tab_id(), reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size())))
            return false;
        if (is_internal_page()) {
            emit_active_tab_state_changed();
            return true;
        }
        widget().show();
        widget().load(parsed_url.release_value());
        sync_view_visibility();
        emit_active_tab_state_changed();
        return true;
    }
    case PhotonBrowserCommandKind_Reload:
        if (!is_internal_page())
            widget().reload();
        return true;
    case PhotonBrowserCommandKind_Back:
        widget().traverse_the_history_by_delta(-1);
        return true;
    case PhotonBrowserCommandKind_Forward:
        widget().traverse_the_history_by_delta(1);
        return true;
    case PhotonBrowserCommandKind_None:
        return false;
    }
    VERIFY_NOT_REACHED();
}

void BrowserView::update_url(uint64_t tab_id, QString const& url)
{
    auto utf8 = url.toUtf8();
    if (photon_browser_set_url(m_state, tab_id, reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size())) == 0)
        return;
    emit browser_state_changed();
    if (tab_id == active_tab_id())
        emit url_changed();
}

void BrowserView::update_title(uint64_t tab_id, QString const& title)
{
    auto utf8 = title.toUtf8();
    if (photon_browser_set_title(m_state, tab_id, reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size())) == 0)
        return;
    emit browser_state_changed();
    if (tab_id == active_tab_id())
        emit title_changed();
}

void BrowserView::update_loading(uint64_t tab_id, bool loading)
{
    if (photon_browser_set_loading(m_state, tab_id, loading) == 0)
        return;
    emit browser_state_changed();
    if (tab_id == active_tab_id())
        emit loading_changed();
}

void BrowserView::update_navigation_capabilities(uint64_t tab_id)
{
    auto* view = m_views.value(tab_id);
    if (!view)
        return;
    if (photon_browser_set_navigation_capabilities(m_state, tab_id, view->navigate_back_action().enabled(), view->navigate_forward_action().enabled()) == 0)
        return;
    emit browser_state_changed();
    if (tab_id == active_tab_id())
        emit navigation_capabilities_changed();
}

void BrowserView::emit_active_tab_state_changed()
{
    sync_view_visibility();
    emit url_changed();
    emit title_changed();
    emit loading_changed();
    emit navigation_capabilities_changed();
    emit browser_state_changed();
}

void BrowserView::sync_view_visibility()
{
    auto active_id = active_tab_id();
    auto show_active = !is_internal_page();
    for (auto it = m_views.cbegin(); it != m_views.cend(); ++it)
        it.value()->setVisible(it.key() == active_id && show_active);
}

}
