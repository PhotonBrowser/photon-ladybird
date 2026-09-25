/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/Platform/WindowEffectsPlatform.h>

namespace Photon {

class UnsupportedWindowEffects final : public PlatformWindowEffects {
public:
    void set_blur_regions(std::vector<QRect> const&) override { }
    bool supports_background_blur() const override { return false; }
};

std::unique_ptr<PlatformWindowEffects> create_platform_window_effects(QWindow&)
{
    return std::make_unique<UnsupportedWindowEffects>();
}

}
