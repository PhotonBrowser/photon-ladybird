/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/ChromeSurface.h>
#include <Photon/Bridge/PhotonApplication.h>
#include <Photon/Bridge/PhotonWindow.h>
#include <Photon/Bridge/WindowScene.h>

#include <UI/Qt/WebContentView.h>

#include <QApplication>
#include <QCloseEvent>
#include <QKeyEvent>
#include <QKeySequence>
#include <QResizeEvent>
#include <QShortcut>
#include <QTimer>
#include <QWindow>

#include <type_traits>
#include <variant>

namespace Photon {

template<typename>
constexpr bool is_unhandled_command = false;

Window::Window(QWidget* parent)
    : QWidget(parent)
    , m_browser(std::make_unique<BrowserView>(*this))
{
    setWindowTitle(QStringLiteral("Photon"));
    resize(1100, 760);

    connect(m_browser.get(), &BrowserView::title_changed, this, [this] {
        auto title = m_browser->title();
        setWindowTitle(title.isEmpty() ? QStringLiteral("Photon") : QStringLiteral("%1 — Photon").arg(title));
    });
}

Window::~Window()
{
    if (qApp)
        qApp->removeEventFilter(this);
    if (m_browser) {
        auto& application = static_cast<Application&>(WebView::Application::the());
        application.clear_active_view();
    }
    delete m_scene;
    m_scene = nullptr;
    m_browser.reset();
}

static bool is_web_shortcut(QKeyEvent const& event)
{
    auto modifiers = event.modifiers() & (Qt::ControlModifier | Qt::MetaModifier | Qt::AltModifier | Qt::ShiftModifier);
    auto key = event.key();
    if ((modifiers == Qt::ControlModifier || modifiers == Qt::MetaModifier)
        && (key == Qt::Key_L || key == Qt::Key_T || key == Qt::Key_W || key == Qt::Key_R))
        return true;
    if (modifiers == Qt::AltModifier && (key == Qt::Key_Left || key == Qt::Key_Right))
        return true;
    if (modifiers == Qt::ControlModifier && (key == Qt::Key_Tab || key == Qt::Key_PageDown || key == Qt::Key_PageUp))
        return true;
    if (modifiers == (Qt::ControlModifier | Qt::ShiftModifier)
        && (key == Qt::Key_Tab || key == Qt::Key_Backtab))
        return true;
    return modifiers == Qt::NoModifier && key == Qt::Key_F5;
}

void Window::install_web_shortcuts()
{
    auto add = [this](Qt::KeyboardModifiers modifiers, Qt::Key key, auto action) {
        auto* shortcut = new QShortcut(QKeySequence(QKeyCombination(modifiers, key)), this);
        shortcut->setContext(Qt::WindowShortcut);
        connect(shortcut, &QShortcut::activated, this, action);
        return shortcut;
    };

    for (auto modifier : { Qt::ControlModifier, Qt::MetaModifier }) {
        add(modifier, Qt::Key_L, [this] { m_scene->focus_address_bar(); });
        add(modifier, Qt::Key_T, [this] { m_browser->create_tab(); });
        auto* close_tab_shortcut = add(modifier, Qt::Key_W, [this] { m_browser->close_tab(m_browser->active_tab_id()); });
        close_tab_shortcut->setAutoRepeat(false);
        add(modifier, Qt::Key_R, [this] { m_browser->reload(); });
    }
    add(Qt::NoModifier, Qt::Key_F5, [this] { m_browser->reload(); });
    add(Qt::AltModifier, Qt::Key_Left, [this] { m_browser->go_back(); });
    add(Qt::AltModifier, Qt::Key_Right, [this] { m_browser->go_forward(); });
    add(Qt::ControlModifier, Qt::Key_Tab, [this] { m_browser->select_adjacent_tab(false); });
    add(Qt::ControlModifier | Qt::ShiftModifier, Qt::Key_Tab, [this] { m_browser->select_adjacent_tab(true); });
    add(Qt::ControlModifier, Qt::Key_PageDown, [this] { m_browser->select_adjacent_tab(false); });
    add(Qt::ControlModifier, Qt::Key_PageUp, [this] { m_browser->select_adjacent_tab(true); });

    // WebContentView accepts ShortcutOverride for page keys by default. Ignore
    // Photon-owned combinations before the page can reserve them.
    qApp->installEventFilter(this);
}

bool Window::initialize()
{
#ifndef Q_OS_MACOS
    setWindowFlag(Qt::FramelessWindowHint);
#endif
    m_scene = new WindowScene(*m_browser, *this);
    m_scene->chrome().on_command = [this](PhotonCommand const& command) { dispatch_command(command); };
    m_scene->load_chrome();
    install_web_shortcuts();
    connect(m_browser.get(), &BrowserView::title_changed, this, [this] {
        auto title = m_browser->title();
        setWindowTitle(title.isEmpty() ? QStringLiteral("Photon") : QStringLiteral("%1 — Photon").arg(title));
    });
    return true;
}

void Window::dispatch_command(PhotonCommand const& command)
{
    if (!std::holds_alternative<SetOverlayCaptureCommand>(command))
        m_scene->clear_open_overlays();

    std::visit([this](auto const& typed_command) {
        using Command = std::decay_t<decltype(typed_command)>;
        if constexpr (std::is_same_v<Command, NavigateCommand>)
            m_browser->navigate(typed_command.url);
        else if constexpr (std::is_same_v<Command, BackCommand>)
            m_browser->go_back();
        else if constexpr (std::is_same_v<Command, ForwardCommand>)
            m_browser->go_forward();
        else if constexpr (std::is_same_v<Command, ReloadCommand>)
            m_browser->reload();
        else if constexpr (std::is_same_v<Command, NewTabCommand>)
            m_browser->create_tab();
        else if constexpr (std::is_same_v<Command, OpenSettingsCommand>)
            m_browser->open_settings();
        else if constexpr (std::is_same_v<Command, SelectTabCommand>)
            m_browser->select_tab(typed_command.tab_id);
        else if constexpr (std::is_same_v<Command, CloseTabCommand>)
            m_browser->close_tab(typed_command.tab_id);
        else if constexpr (std::is_same_v<Command, ReorderTabsCommand>)
            m_browser->reorder_tabs(typed_command.tab_ids);
        else if constexpr (std::is_same_v<Command, SetThemeCommand>)
            m_browser->set_theme_mode(typed_command.mode);
        else if constexpr (std::is_same_v<Command, SetDimOverlaysCommand>)
            m_browser->set_dim_overlays(typed_command.enabled);
        else if constexpr (std::is_same_v<Command, SetOverlayCaptureCommand>)
            m_scene->set_overlay_open(typed_command.open);
        else if constexpr (std::is_same_v<Command, WindowControlCommand>) {
            switch (typed_command.command) {
            case WindowCommand::Minimize:
                showMinimized();
                break;
            case WindowCommand::ToggleMaximize:
                isMaximized() ? showNormal() : showMaximized();
                break;
            case WindowCommand::Close:
                QTimer::singleShot(0, this, [this] { close(); });
                break;
            case WindowCommand::StartSystemMove:
                if (windowHandle())
                    windowHandle()->startSystemMove();
                break;
            }
        } else
            static_assert(is_unhandled_command<Command>, "PhotonCommand is missing a dispatcher branch");
    },
        command);
}

bool Window::eventFilter(QObject* watched, QEvent* event)
{
    if (m_scene && event->type() == QEvent::ShortcutOverride && QApplication::activeWindow() == this
        && is_web_shortcut(*static_cast<QKeyEvent*>(event))) {
        event->ignore();
        return true;
    }
    return QWidget::eventFilter(watched, event);
}

void Window::resizeEvent(QResizeEvent* event)
{
    QWidget::resizeEvent(event);
    if (m_scene)
        m_scene->setGeometry(rect());
}

void Window::closeEvent(QCloseEvent* event)
{
    QWidget::closeEvent(event);
    if (!event->isAccepted() || !m_browser)
        return;

    auto& application = static_cast<Application&>(WebView::Application::the());
    application.clear_active_view();
    delete m_scene;
    m_scene = nullptr;
    m_browser.reset();
}

}
