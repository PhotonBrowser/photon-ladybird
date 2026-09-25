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
#include <LibWebView/CanonicalTraversable.h>
#include <LibWebView/Menu.h>
#include <LibWebView/WebContentPage.h>
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

static PhotonAppEffects dispatch_app_command(PhotonBrowserState* state, uint32_t kind, uint64_t value = 0, QList<uint64_t> const* ids = nullptr, QString const& argument = { })
{
    auto utf8 = argument.toUtf8();
    PhotonAppCommand command {
        .kind = kind,
        .value = value,
        .ids = ids ? ids->constData() : nullptr,
        .ids_len = ids ? static_cast<size_t>(ids->size()) : 0,
        .argument = { reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size()) },
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
    for (auto* view : m_views)
        apply_page_appearance(*view);
    emit browser_state_changed();
}

void BrowserView::apply_page_appearance(Ladybird::WebContentView& view)
{
    view.set_preferred_color_scheme(preferred_color_scheme());
    auto force_dark = photon_browser_force_dark_pages(m_state) ? "on"sv : "off"sv;
    view.traversable().for_each_hosting_page([&](WebView::WebContentPage& page) {
        page.async_debug_request("set-force-dark"sv, force_dark);
    });
}

Ladybird::WebContentView& BrowserView::create_view(uint64_t tab_id)
{
    auto* view = new Ladybird::WebContentView(&m_host);
    m_views.insert(tab_id, view);
    apply_page_appearance(*view);
    view->hide();
    // A cross-site navigation can replace the hosting WebContent process.
    view->on_load_finish = [this, view](URL::URL const&) {
        apply_page_appearance(*view);
    };
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
        apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_PageClosed, tab_id));
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
        PhotonPageObservation observation { 5, tab_id, { reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size()) }, 0, 0 };
        if (photon_app_observe_page(m_state, observation) == 0)
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
    auto effects = dispatch_app_command(m_state, PhotonAppCommandKind_Navigate, 0, nullptr, input);
    apply_app_effects(effects);
    return effects.accepted;
}

void BrowserView::reload()
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_Reload));
}

void BrowserView::go_back()
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_Back));
}

void BrowserView::go_forward()
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_Forward));
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
    apply_app_effects(dispatch_app_command(m_state, previous ? PhotonAppCommandKind_SelectPreviousTab : PhotonAppCommandKind_SelectNextTab));
}

void BrowserView::close_active_tab()
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_CloseActiveTab));
}

bool BrowserView::request_focus_address()
{
    return dispatch_app_command(m_state, PhotonAppCommandKind_FocusAddress).focus_address;
}

void BrowserView::close_tab(uint64_t tab_id)
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_CloseTab, tab_id));
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

void BrowserView::set_force_dark_pages(bool enabled)
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_SetForceDarkPages, enabled));
}

void BrowserView::set_dim_overlays(bool enabled)
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_SetDimOverlays, enabled));
}

void BrowserView::set_window_tint_opacity(uint8_t opacity)
{
    apply_app_effects(dispatch_app_command(m_state, PhotonAppCommandKind_SetWindowTintOpacity, opacity));
}

void BrowserView::apply_app_effects(PhotonAppEffects const& effects)
{
    if (!effects.accepted)
        return;
    if (effects.requested_close_tab_id != 0) {
        // Rust validated the tab and decided a native close request is required.
        if (auto* view = m_views.value(effects.requested_close_tab_id))
            view->request_close();
    }
    switch (effects.navigation_kind) {
    case 0:
        break;
    case 1: {
        auto* view = m_views.value(effects.navigation_tab_id);
        if (!view)
            break;
        auto parsed_url = ak_url_from_qstring(qstring_from_photon_utf8(effects.navigation_target));
        if (!parsed_url.has_value())
            break;
        view->show();
        view->load(parsed_url.release_value());
        break;
    }
    case 2:
        if (auto* view = m_views.value(effects.navigation_tab_id))
            view->reload();
        break;
    case 3:
        if (auto* view = m_views.value(effects.navigation_tab_id))
            view->traverse_the_history_by_delta(-1);
        break;
    case 4:
        if (auto* view = m_views.value(effects.navigation_tab_id))
            view->traverse_the_history_by_delta(1);
        break;
    default:
        VERIFY_NOT_REACHED();
    }
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
    if (effects.theme_changed || effects.force_dark_pages_changed)
        refresh_preferred_color_scheme();
    else if (effects.navigation_kind == 1)
        emit_active_tab_state_changed();
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

void BrowserView::update_url(uint64_t tab_id, QString const& url)
{
    auto utf8 = url.toUtf8();
    PhotonPageObservation observation { 1, tab_id, { reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size()) }, 0, 0 };
    if (photon_app_observe_page(m_state, observation) == 0)
        return;
    emit browser_state_changed();
    if (tab_id == active_tab_id())
        emit url_changed();
}

void BrowserView::update_title(uint64_t tab_id, QString const& title)
{
    auto utf8 = title.toUtf8();
    PhotonPageObservation observation { 2, tab_id, { reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size()) }, 0, 0 };
    if (photon_app_observe_page(m_state, observation) == 0)
        return;
    emit browser_state_changed();
    if (tab_id == active_tab_id())
        emit title_changed();
}

void BrowserView::update_loading(uint64_t tab_id, bool loading)
{
    PhotonPageObservation observation { 3, tab_id, { }, static_cast<uint8_t>(loading), 0 };
    if (photon_app_observe_page(m_state, observation) == 0)
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
    PhotonPageObservation observation { 4, tab_id, { }, static_cast<uint8_t>(view->navigate_back_action().enabled()), static_cast<uint8_t>(view->navigate_forward_action().enabled()) };
    if (photon_app_observe_page(m_state, observation) == 0)
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
