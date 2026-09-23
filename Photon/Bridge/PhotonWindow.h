/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#pragma once

#include <Photon/Bridge/PhotonCommand.h>

#include <QWidget>

#include <memory>

class QCloseEvent;

namespace Photon {

class BrowserView;
class WindowScene;

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
    virtual bool eventFilter(QObject*, QEvent*) override;

private:
    void dispatch_command(PhotonCommand const&);
    void install_web_shortcuts();
    std::unique_ptr<BrowserView> m_browser;
    WindowScene* m_scene { nullptr };
};

}
