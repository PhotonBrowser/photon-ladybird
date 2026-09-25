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

class PlatformWindowEffects {
public:
    virtual ~PlatformWindowEffects() = default;
    virtual void set_blur_regions(std::vector<QRect> const&) = 0;
    virtual bool supports_background_blur() const = 0;
};

std::unique_ptr<PlatformWindowEffects> create_platform_window_effects(QWindow&);

}
