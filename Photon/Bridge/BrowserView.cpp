/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/PhotonCommand.h>

#include <AK/Base64.h>
#include <LibGfx/ImageFormats/PNGWriter.h>
#include <LibWeb/CSS/PreferredColorScheme.h>
#include <LibWebView/Menu.h>
#include <Photon/Rust/PhotonCore.h>
#include <UI/Qt/StringUtils.h>
#include <UI/Qt/WebContentView.h>

#include <QCursor>
#include <QStandardPaths>
#include <QWidget>

namespace Ladybird {

bool is_using_dark_system_theme(QWidget&);

}

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

static PhotonAppEffects dispatch_app_command(PhotonBrowserState* state, uint32_t kind, uint64_t value = 0, QList<uint64_t> const* ids = nullptr)
{
    PhotonAppCommand command {
        .kind = kind,
        .value = value,
        .ids = ids ? ids->constData() : nullptr,
        .ids_len = ids ? static_cast<size_t>(ids->size()) : 0,
    };
    return photon_app_dispatch(state, command);
}

static Web::CSS::PreferredColorScheme to_preferred_color_scheme(uint8_t mode)
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
    , m_state([&host] {
        auto config_path = QStandardPaths::writableLocation(QStandardPaths::AppConfigLocation);
        config_path += QStringLiteral("/config.toml");
        auto utf8_path = config_path.toUtf8();
        return photon_browser_state_new(
            reinterpret_cast<uint8_t const*>(utf8_path.constData()),
            static_cast<size_t>(utf8_path.size()));
    }())
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

Web::CSS::PreferredColorScheme BrowserView::preferred_color_scheme() const
{
    auto mode = photon_browser_theme_mode(m_state);
    if (mode == 0)
        return Ladybird::is_using_dark_system_theme(m_host) ? Web::CSS::PreferredColorScheme::Dark : Web::CSS::PreferredColorScheme::Light;
    return to_preferred_color_scheme(mode);
}

void BrowserView::refresh_preferred_color_scheme()
{
    auto color_scheme = preferred_color_scheme();
    for (auto* view : m_views)
        view->set_preferred_color_scheme(color_scheme);
    emit browser_state_changed();
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
    view->on_enter_tooltip_area = [this, tab_id](ByteString const& tooltip) {
        if (tab_id != active_tab_id())
            return;
        auto text = qstring_from_ak_string(tooltip)
                        .replace(QStringLiteral("\r\n"), QStringLiteral("\n"))
                        .replace(QLatin1Char('\r'), QLatin1Char('\n'));
        emit page_tooltip_changed(move(text), m_host.mapFromGlobal(QCursor::pos()));
    };
    view->on_leave_tooltip_area = [this, tab_id] {
        if (tab_id == active_tab_id())
            emit page_tooltip_cleared();
    };
    view->on_close = [this, tab_id] {
        apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_CloseTab, tab_id));
    };
    view->on_favicon_change = [this, tab_id](Optional<Gfx::Bitmap const&> const& favicon) {
        auto favicon_url = QString { };
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
    auto effects = dispatch_app_command(m_state, PhotonAppCommandKind_CreateTab);
    auto tab_id = effects.created_tab_id;
    apply_app_effects(effects);
    return tab_id;
}

uint64_t BrowserView::open_settings()
{
    auto effects = dispatch_app_command(m_state, PhotonAppCommandKind_OpenSettings);
    auto tab_id = effects.created_tab_id;
    apply_app_effects(effects);
    return tab_id;
}

void BrowserView::select_tab(uint64_t tab_id)
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_SelectTab, tab_id));
}

void BrowserView::select_adjacent_tab(bool previous)
{
    select_tab(photon_browser_adjacent_tab_id(m_state, previous));
}

void BrowserView::close_tab(uint64_t tab_id)
{
    auto* view = m_views.value(tab_id);
    if (!view)
        return;

    // Keep the final native view alive: the Rust state resets the last tab in place.
    if (m_views.size() == 1) {
        apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_CloseTab, tab_id));
        return;
    }

    // WebContent calls on_close only after its top-level traversable has completed closing.
    // That callback removes the Rust tab and its native view.
    view->request_close();
}

void BrowserView::reorder_tabs(QList<uint64_t> const& tab_ids)
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_ReorderTabs, 0, &tab_ids));
}

void BrowserView::set_theme_mode(ThemeMode mode)
{
    auto value = mode == ThemeMode::Light ? 1 : mode == ThemeMode::Dark ? 2
                                                                        : 0;
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_SetTheme, static_cast<uint64_t>(value)));
}

void BrowserView::set_dim_overlays(bool enabled)
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_SetDimOverlays, enabled));
}

void BrowserView::apply_app_effects(PhotonAppEffects const& effects)
{
    if (!effects.accepted)
        return;
    if (effects.created_tab_id != 0)
        create_view(effects.created_tab_id);
    Ladybird::WebContentView* removed_view = nullptr;
    if (effects.removed_tab_id != 0) {
        removed_view = m_views.take(effects.removed_tab_id);
    }
    if (effects.tabs_changed)
        sync_view_visibility();
    auto active_tab_signal_emitted = effects.active_tab_changed && removed_view;
    if (active_tab_signal_emitted)
        emit active_tab_changed();
    if (removed_view)
        removed_view->deleteLater();
    if (effects.theme_changed)
        refresh_preferred_color_scheme();
    else if (effects.active_tab_changed) {
        if (!active_tab_signal_emitted)
            emit active_tab_changed();
        emit_active_tab_state_changed();
    } else if (effects.state_changed)
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
    if (tab_id == active_tab_id()) {
        if (loading)
            emit page_tooltip_cleared();
        emit loading_changed();
    }
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
