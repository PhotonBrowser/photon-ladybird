/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <AK/Debug.h>
#include <LibMain/Main.h>
#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/PhotonApplication.h>
#include <Photon/Bridge/PhotonWindow.h>

#include <QCoreApplication>
#include <QElapsedTimer>
#include <QGuiApplication>
#include <QProcess>
#include <QStyleHints>
#include <QThread>
#include <QTimer>
#include <QUrl>

#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>

#include <cstdint>
#include <string>

namespace Ladybird {

bool is_using_dark_system_theme(QWidget&);

#if defined(AK_OS_LINUX)
enum class GnomeColorScheme {
    Unknown,
    PreferLight,
    PreferDark,
};

static GnomeColorScheme g_gnome_color_scheme { GnomeColorScheme::Unknown };
#endif

bool is_using_dark_system_theme(QWidget& widget)
{
#if defined(AK_OS_LINUX)
    if (g_gnome_color_scheme != GnomeColorScheme::Unknown)
        return g_gnome_color_scheme == GnomeColorScheme::PreferDark;
#endif

    auto color_scheme = QGuiApplication::styleHints()->colorScheme();
    if (color_scheme != Qt::ColorScheme::Unknown)
        return color_scheme == Qt::ColorScheme::Dark;

    auto color = widget.palette().color(widget.backgroundRole());
    auto luma = 0.2126f * color.redF() + 0.7152f * color.greenF() + 0.0722f * color.blueF();
    return luma <= 0.5f;
}

}

namespace {

// Vite's configured port in Photon/WebUI/vite.config.ts. ChromeSurface only
// trusts this exact local origin, so readiness is only meaningful there.
constexpr uint16_t vite_dev_server_port = 5173;

// Mirror the validation in ChromeSurface::load(): only the local Vite origin
// can serve the trusted chrome document. Anything else falls back to the
// bundled production HTML, which needs no readiness wait.
bool vite_dev_server_configured()
{
    auto dev_server = qgetenv("PHOTON_WEBUI_DEV_SERVER");
    if (dev_server.isEmpty())
        return false;
    QUrl url(QString::fromUtf8(dev_server));
    return url.isValid()
        && url.scheme() == QStringLiteral("http")
        && url.host() == QStringLiteral("127.0.0.1")
        && url.port() == vite_dev_server_port;
}

// A bare TCP accept is not enough: Vite opens the port before it serves the
// transformed index.html, so require the document title and client marker.
// This matches the readiness check in Tools/PhotonCLI/src/build.rs.
bool vite_serves_photon_chrome()
{
    int fd = ::socket(AF_INET, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0)
        return false;

    sockaddr_in address { };
    address.sin_family = AF_INET;
    address.sin_port = htons(vite_dev_server_port);
    address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (::connect(fd, reinterpret_cast<sockaddr*>(&address), sizeof(address)) < 0) {
        ::close(fd);
        return false;
    }

    timeval timeout { .tv_sec = 0, .tv_usec = 500 * 1000 };
    ::setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
    ::setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout));

    static constexpr char request[] = "GET / HTTP/1.1\r\nHost: 127.0.0.1:5173\r\nConnection: close\r\n\r\n";
    size_t sent = 0;
    while (sent < sizeof(request) - 1) {
        auto written = ::send(fd, request + sent, sizeof(request) - 1 - sent, MSG_NOSIGNAL);
        if (written <= 0) {
            ::close(fd);
            return false;
        }
        sent += static_cast<size_t>(written);
    }

    std::string response;
    char buffer[4096];
    for (;;) {
        auto received = ::recv(fd, buffer, sizeof(buffer), 0);
        if (received <= 0)
            break;
        response.append(buffer, static_cast<size_t>(received));
        if (response.size() > 1024 * 1024)
            break;
    }
    ::close(fd);

    return response.find("200 OK") != std::string::npos
        && response.find("Photon Chrome") != std::string::npos
        && response.find("/@vite/client") != std::string::npos;
}

// Keep the window hidden until Vite is fully online so the first chrome
// navigation cannot race a half-started dev server. Production bundles load
// from a Qt resource and return immediately. Still opens the window after a
// timeout so a dead dev server surfaces as a load error instead of a hang.
void wait_for_vite_dev_server()
{
    if (!vite_dev_server_configured())
        return;
    QElapsedTimer timer;
    timer.start();
    constexpr qint64 timeout_ms = 20000;
    while (timer.elapsed() < timeout_ms) {
        if (vite_serves_photon_chrome())
            return;
        QThread::msleep(100);
    }
    warnln("Photon: Vite dev server did not become ready within 20s; opening the window anyway");
}

}

ErrorOr<int> ladybird_main(Main::Arguments arguments)
{
    AK::set_rich_debug_enabled(true);
    QCoreApplication::setAttribute(Qt::AA_DontCreateNativeWidgetSiblings);

    auto application = TRY(Photon::Application::create(arguments));

    // In dev mode the chrome document loads from Vite. Wait until Vite serves
    // the transformed document before initializing the window, so window.show()
    // below cannot open onto a half-started dev server.
    wait_for_vite_dev_server();

    Photon::Window window;
    if (!window.initialize())
        return Error::from_string_literal("Photon failed to load its Web UI");

    bool initial_navigation_started = false;
    auto load_initial_page = [&] {
        if (initial_navigation_started)
            return;
        initial_navigation_started = true;
        window.browser().load_initial_url();
    };
    bool wait_for_gnome_theme = false;

#if defined(AK_OS_LINUX)
    if (qEnvironmentVariable("XDG_CURRENT_DESKTOP").contains(QStringLiteral("GNOME"), Qt::CaseInsensitive)) {
        wait_for_gnome_theme = true;
        auto* process = new QProcess(&window);
        auto* timeout = new QTimer(&window);
        timeout->setSingleShot(true);
        QObject::connect(timeout, &QTimer::timeout, &window, [&] {
            load_initial_page();
            process->kill();
        });
        QObject::connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), &window, [&window, process, timeout, &load_initial_page](int exit_code, QProcess::ExitStatus) {
            timeout->stop();
            if (exit_code == 0) {
                auto preference = QString::fromUtf8(process->readAllStandardOutput()).trimmed();
                if (preference == QStringLiteral("'prefer-dark'"))
                    Ladybird::g_gnome_color_scheme = Ladybird::GnomeColorScheme::PreferDark;
                else if (preference == QStringLiteral("'prefer-light'"))
                    Ladybird::g_gnome_color_scheme = Ladybird::GnomeColorScheme::PreferLight;
            }
            if (Ladybird::g_gnome_color_scheme != Ladybird::GnomeColorScheme::Unknown)
                window.browser().refresh_preferred_color_scheme();
            load_initial_page();
            process->deleteLater();
        });
        QObject::connect(process, &QProcess::errorOccurred, &window, [&window, process, timeout, &load_initial_page](QProcess::ProcessError error) {
            if (error != QProcess::FailedToStart)
                return;
            timeout->stop();
            load_initial_page();
            process->deleteLater();
        });
        process->start(QStringLiteral("gsettings"), { QStringLiteral("get"), QStringLiteral("org.gnome.desktop.interface"), QStringLiteral("color-scheme") });
        // gsettings may take longer on a cold session start. Keep the initial
        // page behind the lookup so it receives the system scheme before its
        // first document is created, avoiding a late prefers-color-scheme flip.
        timeout->start(2000);
    }
#endif

    window.show();
    window.browser().prepare_initial_page();
    if (!wait_for_gnome_theme)
        load_initial_page();

    return application->execute();
}
