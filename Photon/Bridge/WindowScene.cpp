/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/ChromeSurface.h>
#include <Photon/Bridge/WindowScene.h>

#include <UI/Qt/WebContentView.h>

#include <algorithm>

#include <QEvent>
#include <QMouseEvent>
#include <QResizeEvent>
#include <QWheelEvent>

namespace Photon {

WindowScene::WindowScene(BrowserView& browser, QWidget& parent)
    : QWidget(&parent)
    , m_browser(browser)
    , m_chrome(new ChromeSurface(*this))
{
    setAttribute(Qt::WA_TranslucentBackground);
    m_browser.widget().setParent(this);
    m_chrome->view().setParent(this);
    // Keep the chrome surface a normal full-window child, but tell Qt that
    // unpainted pixels are part of the composition rather than a background.
    m_chrome->view().setAttribute(Qt::WA_TranslucentBackground);
    m_chrome->view().setAttribute(Qt::WA_NoSystemBackground);
    m_chrome->view().setAutoFillBackground(false);
    m_chrome->view().installEventFilter(this);
    m_browser.widget().show();
    m_chrome->view().show();
    m_chrome->view().raise();
}

WindowScene::~WindowScene()
{
    // BrowserView owns the page view's lifetime. Reparent it before QObject
    // tears down this scene so the scene cannot delete the same QWidget.
    m_browser.widget().setParent(parentWidget());
}

void WindowScene::load_chrome()
{
    m_chrome->load(m_browser);
}

void WindowScene::resizeEvent(QResizeEvent* event)
{
    QWidget::resizeEvent(event);
    m_browser.widget().setGeometry(page_rect());
    m_chrome->view().setGeometry(rect());
}

QRect WindowScene::page_rect() const
{
    auto top = std::min(chrome_toolbar_height, height());
    return { 0, top, width(), std::max(0, height() - top) };
}

bool WindowScene::chrome_owns_point(QPoint point) const
{
    if (point.y() < chrome_toolbar_height)
        return true;
    // The prototype popover is intentionally a native scene input region. Its
    // DOM controls remain responsible for the finer hit testing inside it.
    return QRect(width() - 310, 54, 310, 240).contains(point);
}

void WindowScene::forward_mouse_event(QEvent* event)
{
    if (auto* mouse = dynamic_cast<QMouseEvent*>(event)) {
        auto scene_position = mouse->position().toPoint();
        if (chrome_owns_point(scene_position))
            return;
        auto position = m_browser.widget().mapFrom(this, scene_position);
        QMouseEvent forwarded(
            mouse->type(),
            position,
            mouse->globalPosition(),
            mouse->button(),
            mouse->buttons(),
            mouse->modifiers());
        QCoreApplication::sendEvent(&m_browser.widget(), &forwarded);
        event->accept();
        return;
    }

    if (auto* wheel = dynamic_cast<QWheelEvent*>(event)) {
        auto scene_position = wheel->position().toPoint();
        if (chrome_owns_point(scene_position))
            return;
        auto position = m_browser.widget().mapFrom(this, scene_position);
        QWheelEvent forwarded(
            position,
            wheel->globalPosition(),
            wheel->pixelDelta(),
            wheel->angleDelta(),
            wheel->buttons(),
            wheel->modifiers(),
            wheel->phase(),
            wheel->inverted(),
            wheel->source());
        QCoreApplication::sendEvent(&m_browser.widget(), &forwarded);
        event->accept();
    }
}

bool WindowScene::eventFilter(QObject* watched, QEvent* event)
{
    if (watched != &m_chrome->view())
        return QWidget::eventFilter(watched, event);

    if (event->type() == QEvent::MouseMove || event->type() == QEvent::MouseButtonPress || event->type() == QEvent::MouseButtonRelease) {
        auto* mouse = static_cast<QMouseEvent*>(event);
        if (!chrome_owns_point(mouse->position().toPoint())) {
            forward_mouse_event(event);
            return true;
        }
    }

    if (event->type() == QEvent::Wheel) {
        auto* wheel = static_cast<QWheelEvent*>(event);
        if (!chrome_owns_point(wheel->position().toPoint())) {
            forward_mouse_event(event);
            return true;
        }
    }

    return QWidget::eventFilter(watched, event);
}

}
