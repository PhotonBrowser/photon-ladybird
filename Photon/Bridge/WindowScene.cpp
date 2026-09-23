/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/ChromeSurface.h>
#include <Photon/Bridge/PhotonApplication.h>
#include <Photon/Bridge/WindowScene.h>

#include <UI/Qt/WebContentView.h>

#include <algorithm>
#include <utility>

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
    m_chrome_cursor = m_chrome->view().cursor();
    setAttribute(Qt::WA_TranslucentBackground);
    parent.installEventFilter(this);
    m_active_page_view = &m_browser.widget();
    m_active_page_view->setParent(this);
    m_active_page_view->installEventFilter(this);
    m_active_page_view->setGeometry(page_rect());
    m_active_page_view->setVisible(!m_browser.is_internal_page());
    m_chrome->view().setParent(this);
    auto chrome_cursor_changed = std::move(m_chrome->view().on_cursor_change);
    m_chrome->view().on_cursor_change = [this, chrome_cursor_changed = std::move(chrome_cursor_changed)](Gfx::Cursor const& cursor) {
        if (chrome_cursor_changed)
            chrome_cursor_changed(cursor);
        m_chrome_cursor = m_chrome->view().cursor();
        update_page_cursor();
    };
    QObject::connect(&m_browser, &BrowserView::cursor_changed, this, [this] {
        update_page_cursor();
    });
    QObject::connect(&m_browser, &BrowserView::active_tab_changed, this, [this] {
        m_active_page_view->hide();
        m_active_page_view->removeEventFilter(this);
        m_active_page_view->setParent(parentWidget());
        m_active_page_view = &m_browser.widget();
        m_active_page_view->setParent(this);
        m_active_page_view->installEventFilter(this);
        m_active_page_view->setGeometry(page_rect());
        m_active_page_view->setVisible(!m_browser.is_internal_page());
        m_active_page_view->lower();
        m_chrome->view().raise();
        m_pointer_over_page = false;
        auto& application = static_cast<Application&>(WebView::Application::the());
        if (m_browser.is_internal_page()) {
            m_chrome->view().setFocus(Qt::OtherFocusReason);
            application.set_active_view(m_chrome->view());
        } else if (m_chrome->view().hasFocus()) {
            application.set_active_view(m_chrome->view());
        } else {
            application.set_active_view(*m_active_page_view);
        }
        update_page_cursor();
    });
    QObject::connect(&m_browser, &BrowserView::browser_state_changed, this, [this] {
        m_active_page_view->setVisible(!m_browser.is_internal_page());
        m_chrome->update_state(m_browser);
    });
    // Keep the chrome surface a normal full-window child, but tell Qt that
    // unpainted pixels are part of the composition rather than a background.
    m_chrome->view().setAttribute(Qt::WA_TranslucentBackground);
    m_chrome->view().setAttribute(Qt::WA_NoSystemBackground);
    m_chrome->view().setAutoFillBackground(false);
    m_chrome->view().installEventFilter(this);
    m_chrome->view().show();
    m_chrome->view().raise();
}

WindowScene::~WindowScene()
{
    if (parentWidget())
        parentWidget()->removeEventFilter(this);
    // BrowserView owns the page view's lifetime. Reparent it before QObject
    // tears down this scene so the scene cannot delete the same QWidget.
    if (m_active_page_view)
        m_active_page_view->setParent(parentWidget());
}

void WindowScene::load_chrome()
{
    m_chrome->load(m_browser);
    m_chrome->view().setFocus(Qt::OtherFocusReason);
    auto& application = static_cast<Application&>(WebView::Application::the());
    application.set_active_view(m_chrome->view());
}

void WindowScene::focus_address_bar()
{
    m_chrome->view().setFocus(Qt::ShortcutFocusReason);
    auto& application = static_cast<Application&>(WebView::Application::the());
    application.set_active_view(m_chrome->view());
    m_chrome->focus_address_bar();
}

void WindowScene::set_overlay_open(bool open)
{
    m_overlay_open = open;
    if (!open)
        return;
    m_chrome->view().setFocus(Qt::OtherFocusReason);
    auto& application = static_cast<Application&>(WebView::Application::the());
    application.set_active_view(m_chrome->view());
    if (m_pointer_over_page) {
        QEvent leave(QEvent::Leave);
        QCoreApplication::sendEvent(m_active_page_view, &leave);
        m_pointer_over_page = false;
        update_page_cursor();
    }
}

void WindowScene::clear_open_overlays()
{
    m_overlay_open = false;
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
    if (m_browser.is_internal_page())
        return true;
    if (has_open_overlays())
        return true;
    if (point.y() < chrome_toolbar_height)
        return true;
    return false;
}

bool WindowScene::has_open_overlays() const
{
    return m_overlay_open;
}

void WindowScene::update_page_cursor()
{
    if (m_pointer_over_page)
        m_chrome->view().setCursor(m_browser.widget().cursor());
    else
        m_chrome->view().setCursor(m_chrome_cursor);
}

void WindowScene::forward_mouse_event(QEvent* event)
{
    if (auto* mouse = dynamic_cast<QMouseEvent*>(event)) {
        auto scene_position = mouse->position().toPoint();
        if (chrome_owns_point(scene_position))
            return;
        auto position = m_active_page_view->mapFrom(this, scene_position);
        QMouseEvent forwarded(
            mouse->type(),
            position,
            mouse->globalPosition(),
            mouse->button(),
            mouse->buttons(),
            mouse->modifiers());
        QCoreApplication::sendEvent(m_active_page_view, &forwarded);
        event->accept();
        return;
    }

    if (auto* wheel = dynamic_cast<QWheelEvent*>(event)) {
        auto scene_position = wheel->position().toPoint();
        if (chrome_owns_point(scene_position))
            return;
        auto position = m_active_page_view->mapFrom(this, scene_position);
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
        QCoreApplication::sendEvent(m_active_page_view, &forwarded);
        event->accept();
    }
}

void WindowScene::forward_overlay_event_to_chrome(QEvent* event)
{
    auto& chrome = m_chrome->view();
    if (auto* mouse = dynamic_cast<QMouseEvent*>(event)) {
        auto position = chrome.mapFromGlobal(mouse->globalPosition().toPoint());
        QMouseEvent forwarded(
            mouse->type(),
            position,
            mouse->globalPosition(),
            mouse->button(),
            mouse->buttons(),
            mouse->modifiers());
        QCoreApplication::sendEvent(&chrome, &forwarded);
    } else if (auto* wheel = dynamic_cast<QWheelEvent*>(event)) {
        auto position = chrome.mapFromGlobal(wheel->globalPosition().toPoint());
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
        QCoreApplication::sendEvent(&chrome, &forwarded);
    }
    event->accept();
}

bool WindowScene::eventFilter(QObject* watched, QEvent* event)
{
    if (watched == parentWidget() && event->type() == QEvent::WindowActivate) {
        if (m_browser.is_internal_page() || (!m_active_page_view->hasFocus() && !m_chrome->view().hasFocus()))
            m_chrome->view().setFocus(Qt::OtherFocusReason);
        auto& application = static_cast<Application&>(WebView::Application::the());
        if (m_chrome->view().hasFocus() || m_browser.is_internal_page())
            application.set_active_view(m_chrome->view());
        else
            application.set_active_view(*m_active_page_view);
    }

    if (event->type() == QEvent::FocusIn) {
        auto& application = static_cast<Application&>(WebView::Application::the());
        if (watched == &m_chrome->view())
            application.set_active_view(m_chrome->view());
        else if (watched == m_active_page_view)
            application.set_active_view(*m_active_page_view);
    }

    // Qt can deliver directly to the page child even while the transparent
    // chrome view is raised above it. Keep the page inert until the overlay
    // closes, and let the chrome document handle its popover or scrim.
    if (watched == m_active_page_view && has_open_overlays()) {
        if (event->type() == QEvent::MouseMove || event->type() == QEvent::MouseButtonPress
            || event->type() == QEvent::MouseButtonRelease || event->type() == QEvent::MouseButtonDblClick
            || event->type() == QEvent::Wheel) {
            if (event->type() == QEvent::MouseButtonPress || event->type() == QEvent::MouseButtonDblClick)
                m_chrome->view().setFocus(Qt::MouseFocusReason);
            forward_overlay_event_to_chrome(event);
            return true;
        }
    }

    if (watched != &m_chrome->view())
        return QWidget::eventFilter(watched, event);

    if (event->type() == QEvent::MouseMove || event->type() == QEvent::MouseButtonPress || event->type() == QEvent::MouseButtonRelease || event->type() == QEvent::MouseButtonDblClick) {
        auto* mouse = static_cast<QMouseEvent*>(event);
        auto point = mouse->position().toPoint();
        m_pointer_over_page = !chrome_owns_point(point);
        update_page_cursor();
        if (m_pointer_over_page) {
            if (event->type() == QEvent::MouseButtonPress || event->type() == QEvent::MouseButtonDblClick)
                m_active_page_view->setFocus(Qt::MouseFocusReason);
            forward_mouse_event(event);
            return true;
        }

        if (event->type() == QEvent::MouseButtonPress || event->type() == QEvent::MouseButtonDblClick)
            m_chrome->view().setFocus(Qt::MouseFocusReason);
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
