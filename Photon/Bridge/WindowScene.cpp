/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/BrowserView.h>
#include <Photon/Bridge/ChromeSurface.h>
#include <Photon/Bridge/PhotonApplication.h>
#include <Photon/Bridge/WindowScene.h>

#include <LibWeb/CSS/PreferredColorScheme.h>
#include <UI/Qt/WebContentView.h>

#include <algorithm>
#include <utility>

#include <QColor>
#include <QEvent>
#include <QGuiApplication>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPalette>
#include <QResizeEvent>
#include <QStyleHints>
#include <QWheelEvent>

namespace Photon {

WindowScene::WindowScene(BrowserView& browser, QWidget& parent)
    : QWidget(&parent)
    , m_browser(browser)
    , m_chrome(new ChromeSurface(*this))
{
    m_chrome_cursor = m_chrome->view().cursor();
    setAttribute(Qt::WA_TranslucentBackground);
    setAttribute(Qt::WA_NoSystemBackground);
    setAutoFillBackground(false);
    update_background_color();
    parent.installEventFilter(this);
    m_active_page_view = &m_browser.widget();
    m_chrome->view().set_preferred_color_scheme(m_browser.preferred_color_scheme());
    m_active_page_view->setParent(this);
    m_active_page_view->installEventFilter(this);
    update_page_geometry();
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
        m_chrome->clear_page_tooltip();
        m_active_page_view->hide();
        m_active_page_view->removeEventFilter(this);
        m_active_page_view->setParent(parentWidget());
        m_active_page_view = &m_browser.widget();
        m_active_page_view->setParent(this);
        m_active_page_view->installEventFilter(this);
        update_page_geometry();
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
        m_chrome->view().set_preferred_color_scheme(m_browser.preferred_color_scheme());
        update_background_color();
        m_chrome->update_state(m_browser);
    });
    QObject::connect(&m_browser, &BrowserView::page_tooltip_changed, this, [this](QString text, QPoint position) {
        m_chrome->update_page_tooltip(text, position);
    });
    QObject::connect(&m_browser, &BrowserView::page_tooltip_cleared, this, [this] {
        m_chrome->clear_page_tooltip();
    });
    QObject::connect(QGuiApplication::styleHints(), &QStyleHints::colorSchemeChanged, this, [this](Qt::ColorScheme) {
        m_browser.refresh_preferred_color_scheme();
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
    if (open)
        m_chrome->clear_page_tooltip();
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
    update_page_geometry();
    m_chrome->view().setGeometry(rect());
}

void WindowScene::paintEvent(QPaintEvent*)
{
    QPainter painter(this);
    // QWidget backing stores can retain the parent palette under transparent
    // WebContent pixels. Clear the backing store itself so the native surface
    // alpha reaches the Wayland compositor.
    painter.setCompositionMode(QPainter::CompositionMode_Source);
    painter.fillRect(rect(), Qt::transparent);
    painter.setCompositionMode(QPainter::CompositionMode_SourceOver);
    // Ordinary webpages keep their normal opaque canvas fallback. Photon
    // internal pages can expose the compositor effect across the full window.
    if (!m_browser.is_internal_page())
        painter.fillRect(page_view_rect(), palette().color(QPalette::Window));
}

QRect WindowScene::page_rect() const
{
    auto top = std::min(chrome_toolbar_height, height());
    auto horizontal_inset = std::min(page_inset, width() / 2);
    auto vertical_inset = std::min(page_inset, std::max(0, height() - top) / 2);
    return { horizontal_inset, top + vertical_inset, std::max(0, width() - horizontal_inset * 2), std::max(0, height() - top - vertical_inset * 2) };
}

QRect WindowScene::page_view_rect() const
{
    auto top = std::min(chrome_toolbar_height, height());
    return { 0, top, width(), std::max(0, height() - top) };
}

void WindowScene::update_background_color()
{
    // Keep these canvas colors aligned with --photon-color-canvas in styles.css.
    auto color = m_browser.preferred_color_scheme() == Web::CSS::PreferredColorScheme::Dark
        ? QColor("#1e2023")
        : QColor("#f4f5f6");
    auto scene_palette = palette();
    scene_palette.setColor(QPalette::Window, color);
    setPalette(scene_palette);
    update();
}

void WindowScene::update_page_geometry()
{
    m_active_page_view->setGeometry(page_view_rect());
}

bool WindowScene::page_contains(QPoint point) const
{
    if (!page_rect().contains(point))
        return false;

    QPainterPath page;
    auto visible_rect = QRectF(page_rect().translated(-m_active_page_view->geometry().topLeft()));
    page.addRoundedRect(visible_rect, page_corner_radius, page_corner_radius);
    return page.contains(m_active_page_view->mapFrom(this, point));
}

bool WindowScene::chrome_owns_point(QPoint point) const
{
    if (m_browser.is_internal_page())
        return true;
    if (has_open_overlays())
        return true;
    if (!page_contains(point))
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

    if (watched == m_active_page_view) {
        QPoint point;
        if (auto* mouse = dynamic_cast<QMouseEvent*>(event))
            point = m_active_page_view->mapTo(this, mouse->position().toPoint());
        else if (auto* wheel = dynamic_cast<QWheelEvent*>(event))
            point = m_active_page_view->mapTo(this, wheel->position().toPoint());
        else
            point = { -1, -1 };

        auto* mouse = dynamic_cast<QMouseEvent*>(event);
        auto scene_position = mouse ? m_active_page_view->mapTo(this, mouse->position().toPoint()) : QPoint { -1, -1 };
        if (mouse && event->type() == QEvent::MouseButtonPress && page_contains(scene_position) && !has_open_overlays())
            m_chrome->blur_address_bar();
        auto is_titlebar_double_click = mouse && event->type() == QEvent::MouseButtonDblClick
            && mouse->button() == Qt::LeftButton && scene_position.y() >= 0 && scene_position.y() < 36;

        if ((event->type() == QEvent::MouseMove || event->type() == QEvent::MouseButtonPress
                || event->type() == QEvent::MouseButtonRelease || event->type() == QEvent::MouseButtonDblClick
                || event->type() == QEvent::Wheel)
            && !page_contains(point) && !is_titlebar_double_click)
            return true;

        if (is_titlebar_double_click) {
            auto& chrome = m_chrome->view();
            QMouseEvent forwarded(QEvent::MouseButtonDblClick, chrome.mapFrom(this, scene_position), mouse->globalPosition(), Qt::LeftButton, mouse->buttons(), mouse->modifiers());
            QCoreApplication::sendEvent(&chrome, &forwarded);
            event->accept();
            return true;
        }
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
            if (event->type() == QEvent::MouseButtonPress || event->type() == QEvent::MouseButtonDblClick) {
                m_chrome->blur_address_bar();
                m_active_page_view->setFocus(Qt::MouseFocusReason);
            }
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
