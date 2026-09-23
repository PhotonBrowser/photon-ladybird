/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
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
#include <QQuickItem>
#include <QQuickWidget>
#include <QResizeEvent>
#include <QShortcut>
#include <QTimer>
#include <QVariant>

namespace Photon {

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
    delete m_quick_view;
    m_quick_view = nullptr;
    m_surface_item = nullptr;
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
    };

    for (auto modifier : { Qt::ControlModifier, Qt::MetaModifier }) {
        add(modifier, Qt::Key_L, [this] { m_scene->focus_address_bar(); });
        add(modifier, Qt::Key_T, [this] { m_browser->create_tab(); });
        add(modifier, Qt::Key_W, [this] { m_browser->close_tab(m_browser->active_tab_id()); });
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

bool Window::initialize(bool web_ui)
{
    if (web_ui) {
        m_scene = new WindowScene(*m_browser, *this);
        m_scene->chrome().on_command = [this](QString const& command, QString const& value) {
            if (command != QStringLiteral("capture"))
                m_scene->clear_open_overlays();
            if (command == QStringLiteral("navigate"))
                m_browser->navigate(value);
            else if (command == QStringLiteral("back"))
                m_browser->go_back();
            else if (command == QStringLiteral("forward"))
                m_browser->go_forward();
            else if (command == QStringLiteral("reload"))
                m_browser->reload();
            else if (command == QStringLiteral("new-tab"))
                m_browser->create_tab();
            else if (command == QStringLiteral("open-settings"))
                m_browser->open_settings();
            else if (command == QStringLiteral("select-tab"))
                m_browser->select_tab(value.mid(4).toULongLong());
            else if (command == QStringLiteral("close-tab"))
                m_browser->close_tab(value.mid(4).toULongLong());
            else if (command == QStringLiteral("reorder-tabs")) {
                QList<uint64_t> tab_ids;
                for (auto const& tab_id : value.split(QLatin1Char(',')))
                    tab_ids.append(tab_id.mid(4).toULongLong());
                m_browser->reorder_tabs(tab_ids);
            } else if (command == QStringLiteral("set-theme"))
                m_browser->set_theme_mode(value);
            else if (command == QStringLiteral("capture")) {
                m_scene->set_overlay_open(value.endsWith(QStringLiteral(":open")));
            }
        };
        m_scene->load_chrome();
        install_web_shortcuts();
        connect(m_browser.get(), &BrowserView::title_changed, this, [this] {
            auto title = m_browser->title();
            setWindowTitle(title.isEmpty() ? QStringLiteral("Photon") : QStringLiteral("%1 — Photon").arg(title));
        });
        return true;
    }

    m_quick_view = new QQuickWidget(this);
    m_quick_view->setResizeMode(QQuickWidget::SizeRootObjectToView);
    m_quick_view->setInitialProperties({ { QStringLiteral("browserState"), QVariant::fromValue(static_cast<QObject*>(m_browser.get())) } });
    m_quick_view->setSource(QUrl(QStringLiteral("qrc:/Photon/UI/Main.qml")));
    if (m_quick_view->status() == QQuickWidget::Error)
        return false;

    m_surface_item = m_quick_view->rootObject()->findChild<QQuickItem*>(QStringLiteral("browserSurface"));
    if (!m_surface_item)
        return false;

    auto update_geometry = [this] { update_web_surface_geometry(); };
    connect(m_surface_item, &QQuickItem::xChanged, this, update_geometry);
    connect(m_surface_item, &QQuickItem::yChanged, this, update_geometry);
    connect(m_surface_item, &QQuickItem::widthChanged, this, update_geometry);
    connect(m_surface_item, &QQuickItem::heightChanged, this, update_geometry);

    m_quick_view->show();
    m_browser->widget().setVisible(!m_browser->is_internal_page());
    if (!m_browser->is_internal_page())
        m_browser->widget().raise();
    connect(m_browser.get(), &BrowserView::active_tab_changed, this, [this] {
        update_web_surface_geometry();
        auto& page = m_browser->widget();
        if (!m_browser->is_internal_page())
            page.raise();
        auto& application = static_cast<Application&>(WebView::Application::the());
        application.set_active_view(page);
    });
    connect(m_browser.get(), &BrowserView::browser_state_changed, this, [this] {
        auto& page = m_browser->widget();
        page.setVisible(!m_browser->is_internal_page());
        if (!m_browser->is_internal_page())
            page.raise();
    });
    m_browser->widget().raise();
    QTimer::singleShot(0, this, update_geometry);
    return true;
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
    if (m_scene) {
        m_scene->setGeometry(rect());
        return;
    }
    m_quick_view->setGeometry(rect());
    update_web_surface_geometry();
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
    delete m_quick_view;
    m_quick_view = nullptr;
    m_surface_item = nullptr;
    m_browser.reset();
}

void Window::update_web_surface_geometry()
{
    if (!m_surface_item)
        return;

    auto origin = m_surface_item->mapToScene(QPointF(0, 0));
    m_browser->widget().setGeometry(
        qRound(origin.x()),
        qRound(origin.y()),
        qRound(m_surface_item->width()),
        qRound(m_surface_item->height()));
}

}
