/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/Platform/WindowEffectsPlatform.h>
#include <Photon/Bridge/WindowEffects.h>

namespace Photon {

WindowEffects::WindowEffects(QWindow& window)
    : m_platform_effects(create_platform_window_effects(window))
{
}

WindowEffects::~WindowEffects() = default;

void WindowEffects::set_blur_regions(std::vector<QRect> const& regions)
{
    m_platform_effects->set_blur_regions(regions);
}

void WindowEffects::clear_blur_regions()
{
    m_platform_effects->set_blur_regions(std::vector<QRect> { });
}

bool WindowEffects::supports_background_blur() const
{
    return m_platform_effects->supports_background_blur();
}

}
