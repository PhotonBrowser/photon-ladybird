/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#include <QWidget>

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

protected:
    virtual void resizeEvent(QResizeEvent*) override;
    virtual bool eventFilter(QObject*, QEvent*) override;

private:
    static constexpr int chrome_toolbar_height = 42;

    QRect page_rect() const;
    bool chrome_owns_point(QPoint) const;
    void forward_mouse_event(QEvent*);

    BrowserView& m_browser;
    ChromeSurface* m_chrome { nullptr };
};

}
