/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <Photon/Bridge/BrowserView.h>

#include <LibWebView/Menu.h>
#include <Photon/Rust/PhotonCore.h>
#include <UI/Qt/StringUtils.h>
#include <UI/Qt/WebContentView.h>

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
    , m_view(new Ladybird::WebContentView(&host))
    , m_state(photon_browser_state_new())
{
    m_view->on_url_change = [this](URL::URL const& url) {
        update_url(qstring_from_ak_string(url.serialize()));
    };
    m_view->on_title_change = [this](Utf16String const& title) {
        update_title(qstring_from_utf16_string(title));
    };
    m_view->on_loading_state_change = [this](bool loading) {
        update_loading(loading);
    };

    m_view->navigate_back_action().add_observer(adopt_own(*new NavigationActionObserver([this] {
        update_navigation_capabilities();
    })));
    m_view->navigate_forward_action().add_observer(adopt_own(*new NavigationActionObserver([this] {
        update_navigation_capabilities();
    })));
}

BrowserView::~BrowserView()
{
    delete m_view;
    m_view = nullptr;
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
    m_view->setFocus(Qt::ShortcutFocusReason);
}

bool BrowserView::apply_command(PhotonBrowserCommand const& command)
{
    switch (command.kind) {
    case PhotonBrowserCommandKind_Navigate: {
        auto url = ak_url_from_qstring(qstring_from_photon_utf8(command.argument));
        if (!url.has_value())
            return false;
        m_view->load(url.release_value());
        return true;
    }
    case PhotonBrowserCommandKind_Reload:
        m_view->reload();
        return true;
    case PhotonBrowserCommandKind_Back:
        m_view->traverse_the_history_by_delta(-1);
        return true;
    case PhotonBrowserCommandKind_Forward:
        m_view->traverse_the_history_by_delta(1);
        return true;
    case PhotonBrowserCommandKind_None:
        return false;
    }
    VERIFY_NOT_REACHED();
}

void BrowserView::update_url(QString const& url)
{
    auto utf8 = url.toUtf8();
    if (photon_browser_set_url(m_state, reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size())) != 0)
        emit url_changed();
}

void BrowserView::update_title(QString const& title)
{
    auto utf8 = title.toUtf8();
    if (photon_browser_set_title(m_state, reinterpret_cast<uint8_t const*>(utf8.constData()), static_cast<size_t>(utf8.size())) != 0)
        emit title_changed();
}

void BrowserView::update_loading(bool loading)
{
    if (photon_browser_set_loading(m_state, loading) != 0)
        emit loading_changed();
}

void BrowserView::update_navigation_capabilities()
{
    if (photon_browser_set_navigation_capabilities(
            m_state,
            m_view->navigate_back_action().enabled(),
            m_view->navigate_forward_action().enabled())
        != 0)
        emit navigation_capabilities_changed();
}

void BrowserView::load_initial_url()
{
    if (!navigate(url()))
        warnln("Photon: Rust core supplied an invalid initial URL");
}

}
