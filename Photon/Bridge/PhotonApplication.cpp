/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/PhotonApplication.h>

#include <UI/Qt/EventLoopImplementationQt.h>
#include <UI/Qt/StringUtils.h>
#include <UI/Qt/WebContentView.h>

#include <QApplication>
#include <QFont>
#include <QGuiApplication>

namespace Photon {

Application::~Application() = default;

Core::EventLoop& Application::create_platform_event_loop()
{
    Core::EventLoopManager::install(*new Ladybird::EventLoopManagerQt);
    m_qt_application = std::make_unique<QApplication>(arguments().argc, arguments().argv);
    QCoreApplication::setApplicationName(QStringLiteral("Photon"));
    QCoreApplication::setOrganizationName(QStringLiteral("Photon"));

    auto& event_loop = WebView::Application::create_platform_event_loop();
    static_cast<Ladybird::EventLoopImplementationQt&>(event_loop.impl()).set_main_loop();
    return event_loop;
}

Optional<String> Application::ui_font_family() const
{
    return ak_string_from_qstring(QGuiApplication::font().family());
}

Optional<WebView::ViewImplementation&> Application::active_web_view() const
{
    if (!m_active_view)
        return { };
    return *m_active_view;
}

Vector<WebView::ViewImplementation&> Application::active_window_web_views() const
{
    if (!m_active_view)
        return { };
    Vector<WebView::ViewImplementation&> views;
    views.append(*m_active_view);
    return views;
}

}
