/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <LibMain/Main.h>
#include <AK/Debug.h>
#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/PhotonApplication.h>
#include <Photon/Bridge/PhotonWindow.h>

#include <QCoreApplication>
#include <QGuiApplication>
#include <QProcess>
#include <QStyleHints>

#include <cstdlib>
#include <cstring>

namespace Ladybird {

bool is_using_dark_system_theme(QWidget&);

bool is_using_dark_system_theme(QWidget& widget)
{
#if defined(AK_OS_LINUX)
    if (qEnvironmentVariable("XDG_CURRENT_DESKTOP").contains(QStringLiteral("GNOME"), Qt::CaseInsensitive)) {
        QProcess process;
        process.start(QStringLiteral("gsettings"), { QStringLiteral("get"), QStringLiteral("org.gnome.desktop.interface"), QStringLiteral("color-scheme") });
        if (process.waitForFinished(1000) && process.exitCode() == 0) {
            auto preference = QString::fromUtf8(process.readAllStandardOutput()).trimmed();
            if (preference == QStringLiteral("'prefer-dark'"))
                return true;
            if (preference == QStringLiteral("'prefer-light'"))
                return false;
        }
    }
#endif

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

    auto selected_ui = getenv("PHOTON_UI");
    bool qml_ui = selected_ui && StringView(selected_ui, strlen(selected_ui)) == "qml"sv;
    bool web_ui = !qml_ui;
    if (qml_ui)
        warnln("Photon: the QML UI is deprecated and will be removed after the React Web UI migration");

    Photon::Window window;
    if (!window.initialize(web_ui)) {
        if (web_ui)
            return Error::from_string_literal("Photon failed to load its Web UI");
        return Error::from_string_literal("Photon failed to load its QML interface");
    }

    application->set_active_view(window.browser().widget());
    window.show();
    if (qml_ui)
        window.browser().navigate(QStringLiteral("https://example.com"));
    else
        window.browser().load_initial_url();

    return application->execute();
}
