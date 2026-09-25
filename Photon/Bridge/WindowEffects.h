/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#pragma once

#include <QRect>
#include <QWindow>

#include <memory>
#include <vector>

namespace Photon {

class PlatformWindowEffects;

// Blur rectangles use QWindow's surface-local logical coordinates.
class WindowEffects final {
public:
    explicit WindowEffects(QWindow&);
    ~WindowEffects();

    WindowEffects(WindowEffects const&) = delete;
    WindowEffects& operator=(WindowEffects const&) = delete;

    void set_blur_regions(std::vector<QRect> const&);
    void clear_blur_regions();
    bool supports_background_blur() const;

private:
    std::unique_ptr<PlatformWindowEffects> m_platform_effects;
};

}
