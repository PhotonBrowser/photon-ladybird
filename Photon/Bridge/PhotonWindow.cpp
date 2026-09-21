/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/PhotonApplication.h>
#include <Photon/Bridge/PhotonWindow.h>

#include <UI/Qt/WebContentView.h>

#include <QCloseEvent>
#include <QQuickItem>
#include <QQuickWidget>
#include <QResizeEvent>
#include <QTimer>
#include <QVariant>

namespace Photon {

Window::Window(QWidget* parent)
    : QWidget(parent)
    , m_browser(std::make_unique<BrowserView>(*this))
    , m_quick_view(new QQuickWidget(this))
{
    setWindowTitle(QStringLiteral("Photon"));
    resize(1100, 760);

    connect(m_browser.get(), &BrowserView::title_changed, this, [this] {
        auto title = m_browser->title();
        setWindowTitle(title.isEmpty() ? QStringLiteral("Photon") : QStringLiteral("%1 — Photon").arg(title));
    });
}

Window::~Window() = default;

bool Window::initialize()
{
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
    m_browser->widget().show();
    m_browser->widget().raise();
    QTimer::singleShot(0, this, update_geometry);
    return true;
}

void Window::resizeEvent(QResizeEvent* event)
{
    QWidget::resizeEvent(event);
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
