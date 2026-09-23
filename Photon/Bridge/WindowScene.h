/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#include <QCursor>
#include <QRect>
#include <QString>
#include <QWidget>

namespace Ladybird {

class WebContentView;

}

namespace Photon {

class BrowserView;
class ChromeSurface;

class WindowScene final : public QWidget {
    Q_OBJECT
public:
    explicit WindowScene(BrowserView&, QWidget& parent);
    virtual ~WindowScene() override;

    ChromeSurface& chrome() { return *m_chrome; }
    void load_chrome();
    void focus_address_bar();
    void set_overlay_open(bool open);
    void clear_open_overlays();

protected:
    virtual void resizeEvent(QResizeEvent*) override;
    virtual bool eventFilter(QObject*, QEvent*) override;

private:
    static constexpr int chrome_toolbar_height = 72;

    QRect page_rect() const;
    bool chrome_owns_point(QPoint) const;
    bool has_open_overlays() const;
    void forward_mouse_event(QEvent*);
    void forward_overlay_event_to_chrome(QEvent*);
    void update_page_cursor();

    BrowserView& m_browser;
    ChromeSurface* m_chrome { nullptr };
    bool m_pointer_over_page { false };
    QCursor m_chrome_cursor;
    Ladybird::WebContentView* m_active_page_view { nullptr };
    bool m_overlay_open { false };
};

}
