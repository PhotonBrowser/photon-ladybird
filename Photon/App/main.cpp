/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <LibMain/Main.h>
#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/PhotonApplication.h>
#include <Photon/Bridge/PhotonWindow.h>

#include <QCoreApplication>
#include <QGuiApplication>
#include <QStyleHints>

namespace Ladybird {

bool is_using_dark_system_theme(QWidget&);

bool is_using_dark_system_theme(QWidget& widget)
{
    auto color_scheme = QGuiApplication::styleHints()->colorScheme();
    if (color_scheme != Qt::ColorScheme::Unknown)
        return color_scheme == Qt::ColorScheme::Dark;

    auto color = widget.palette().color(widget.backgroundRole());
    auto luma = 0.2126f * color.redF() + 0.7152f * color.greenF() + 0.0722f * color.blueF();
    return luma <= 0.5f;
}

}

ErrorOr<int> ladybird_main(Main::Arguments arguments)
{
    AK::set_rich_debug_enabled(true);
    QCoreApplication::setAttribute(Qt::AA_DontCreateNativeWidgetSiblings);

    auto application = TRY(Photon::Application::create(arguments));

    Photon::Window window;
    if (!window.initialize())
        return Error::from_string_literal("Photon failed to load its QML interface");

    application->set_active_view(window.browser().widget());
    window.show();
    window.browser().load_initial_url();

    return application->execute();
}
