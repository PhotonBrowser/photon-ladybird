/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#include <QWidget>

#include <memory>

class QQuickItem;
class QQuickWidget;
class QCloseEvent;

namespace Photon {

class BrowserView;

class Window final : public QWidget {
    Q_OBJECT

public:
    explicit Window(QWidget* parent = nullptr);
    virtual ~Window() override;

    bool initialize();
    BrowserView& browser() const { return *m_browser; }

protected:
    virtual void closeEvent(QCloseEvent*) override;
    virtual void resizeEvent(QResizeEvent*) override;

private:
    void update_web_surface_geometry();

    std::unique_ptr<BrowserView> m_browser;
    QQuickWidget* m_quick_view { nullptr };
    QQuickItem* m_surface_item { nullptr };
};

}
