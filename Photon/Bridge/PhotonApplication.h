/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#pragma once

#include <LibWebView/Application.h>

#include <QApplication>

#include <memory>

namespace Ladybird {

class WebContentView;

}

namespace Photon {

class Application final : public WebView::Application {
    WEB_VIEW_APPLICATION(Application)

public:
    virtual ~Application() override;

    void set_active_view(Ladybird::WebContentView& view) { m_active_view = &view; }
    void clear_active_view() { m_active_view = nullptr; }

private:
    Application() = default;

    virtual Core::EventLoop& create_platform_event_loop() override;
    virtual Optional<String> ui_font_family() const override;
    virtual Optional<WebView::ViewImplementation&> active_web_view() const override;
    virtual Vector<WebView::ViewImplementation&> active_window_web_views() const override;
    virtual bool should_coordinate_browser_process() const override { return false; }
    virtual bool should_use_temporary_profile_by_default() const override { return true; }

    std::unique_ptr<QApplication> m_qt_application;
    Ladybird::WebContentView* m_active_view { nullptr };
};

}
