/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/Platform/WindowEffectsPlatform.h>

#include "ext-background-effect-v1-client-protocol.h"

#include <QGuiApplication>
#include <QLoggingCategory>
#include <QRegion>
#include <qguiapplication_platform.h>
#include <qpa/qplatformwindow_p.h>
#include <wayland-client.h>

#include <algorithm>
#include <cerrno>
#include <cstring>

namespace Photon {

Q_STATIC_LOGGING_CATEGORY(log_window_effects, "photon.window-effects")

class LinuxWindowEffects final : public QObject
    , public PlatformWindowEffects {
public:
    explicit LinuxWindowEffects(QWindow& window)
        : QObject(&window)
        , m_window(window)
    {
        auto* application = qobject_cast<QGuiApplication*>(QCoreApplication::instance());
        auto* wayland_application = application
            ? application->nativeInterface<QNativeInterface::QWaylandApplication>()
            : nullptr;
        auto* wayland_window = window.nativeInterface<QNativeInterface::Private::QWaylandWindow>();
        if (!wayland_application || !wayland_window) {
            qCInfo(log_window_effects) << "Wayland unavailable; background blur is unsupported";
            return;
        }

        m_display = wayland_application->display();
        m_compositor = wayland_application->compositor();
        if (!m_display || !m_compositor) {
            qCInfo(log_window_effects) << "Wayland detected, but Qt did not expose its display/compositor";
            return;
        }

        qCInfo(log_window_effects) << "Wayland detected for Photon window effects";
        m_wayland_window = wayland_window;
        m_surface_created_connection = QObject::connect(wayland_window, &QNativeInterface::Private::QWaylandWindow::surfaceCreated,
            &window, [this] {
                m_surface = m_wayland_window->surface();
                attach_surface();
            });
        m_surface_destroyed_connection = QObject::connect(wayland_window, &QNativeInterface::Private::QWaylandWindow::surfaceDestroyed,
            &window, [this] {
                detach_effect();
                m_surface = nullptr;
            });
        m_surface = wayland_window->surface();

        m_registry = wl_display_get_registry(m_display);
        static constexpr wl_registry_listener registry_listener {
            .global = on_global,
            .global_remove = on_global_remove,
        };
        wl_registry_add_listener(m_registry, &registry_listener, this);

        // The registry belongs to Qt's existing connection. A roundtrip here
        // obtains initial globals/capabilities before the window is shown.
        if (wl_display_roundtrip(m_display) < 0) {
            qCWarning(log_window_effects) << "Wayland registry roundtrip failed";
            return;
        }
        if (!m_manager)
            qCInfo(log_window_effects) << "ext_background_effect_manager_v1 not advertised by this compositor";
    }

    ~LinuxWindowEffects() override
    {
        QObject::disconnect(m_surface_created_connection);
        QObject::disconnect(m_surface_destroyed_connection);
        detach_effect();
        if (m_manager)
            ext_background_effect_manager_v1_destroy(m_manager);
        if (m_registry)
            wl_registry_destroy(m_registry);
        flush();
    }

    void set_blur_regions(std::vector<QRect> const& rectangles) override
    {
        QRegion requested;
        auto surface_size = m_window.size();
        QRect surface_bounds(QPoint(0, 0), surface_size);
        for (auto const& rectangle : rectangles) {
            if (rectangle.width() <= 0 || rectangle.height() <= 0)
                continue;
            auto clipped = rectangle.intersected(surface_bounds);
            if (!clipped.isEmpty())
                requested += clipped;
        }
        if (requested == m_regions)
            return;

        m_regions = requested;
        auto bounds = m_regions.boundingRect();
        qCInfo(log_window_effects) << "Blur region updated:" << m_regions.rectCount()
                                   << "rectangles, bounds" << bounds;
        apply_region();
    }

    bool supports_background_blur() const override
    {
        return m_manager && m_blur_supported;
    }

private:
    static void on_global(void* data, wl_registry* registry, uint32_t name, char const* interface, uint32_t version)
    {
        auto& self = *static_cast<LinuxWindowEffects*>(data);
        if (std::strcmp(interface, ext_background_effect_manager_v1_interface.name) != 0 || self.m_manager)
            return;

        auto bound_version = std::min(version, 1u);
        self.m_manager = static_cast<ext_background_effect_manager_v1*>(wl_registry_bind(
            registry, name, &ext_background_effect_manager_v1_interface, bound_version));
        self.m_manager_name = name;
        static constexpr ext_background_effect_manager_v1_listener manager_listener {
            .capabilities = on_capabilities,
        };
        ext_background_effect_manager_v1_add_listener(self.m_manager, &manager_listener, &self);
        qCInfo(log_window_effects) << "ext_background_effect_manager_v1 detected";
    }

    static void on_global_remove(void* data, wl_registry*, uint32_t name)
    {
        auto& self = *static_cast<LinuxWindowEffects*>(data);
        if (!self.m_manager || name != self.m_manager_name)
            return;
        self.detach_effect();
        ext_background_effect_manager_v1_destroy(self.m_manager);
        self.m_manager = nullptr;
        self.m_manager_name = 0;
        self.update_capability(false);
        qCInfo(log_window_effects) << "ext_background_effect_manager_v1 removed";
    }

    static void on_capabilities(void* data, ext_background_effect_manager_v1*, uint32_t capabilities)
    {
        auto& self = *static_cast<LinuxWindowEffects*>(data);
        self.update_capability(capabilities & EXT_BACKGROUND_EFFECT_MANAGER_V1_CAPABILITY_BLUR);
    }

    void update_capability(bool available)
    {
        if (m_capabilities_received && available == m_blur_supported)
            return;
        m_capabilities_received = true;
        m_blur_supported = available;
        qCInfo(log_window_effects) << "Background blur capability" << (available ? "available" : "unavailable");
        if (!available)
            return;
        if (m_effect)
            apply_region();
        else
            attach_surface();
    }

    void attach_surface()
    {
        if (!m_manager || !m_blur_supported || !m_surface || m_effect)
            return;
        m_effect = ext_background_effect_manager_v1_get_background_effect(m_manager, m_surface);
        if (!m_effect) {
            qCWarning(log_window_effects) << "Failed to attach background effect to Photon wl_surface";
            return;
        }
        qCInfo(log_window_effects) << "Background effect attached to Photon wl_surface";
        apply_region();
    }

    void detach_effect()
    {
        if (!m_effect)
            return;
        ext_background_effect_surface_v1_destroy(m_effect);
        m_effect = nullptr;
        flush();
    }

    void apply_region()
    {
        if (!supports_background_blur() || !m_effect)
            return;

        auto* region = wl_compositor_create_region(m_compositor);
        if (!region) {
            qCWarning(log_window_effects) << "Failed to create Wayland blur region";
            return;
        }
        for (auto const& rectangle : m_regions.rects()) {
            wl_region_add(region, rectangle.x(), rectangle.y(), rectangle.width(), rectangle.height());
        }
        ext_background_effect_surface_v1_set_blur_region(m_effect, region);
        wl_region_destroy(region);
        // Qt owns wl_surface.commit(); its next regular frame applies this
        // double-buffered state. Never commit the shared surface here.
        flush();
    }

    void flush()
    {
        if (m_display && wl_display_flush(m_display) < 0 && errno != EAGAIN)
            qCWarning(log_window_effects) << "Failed to flush Wayland background effect requests";
    }

    QWindow& m_window;
    QNativeInterface::Private::QWaylandWindow* m_wayland_window { nullptr };
    QMetaObject::Connection m_surface_created_connection;
    QMetaObject::Connection m_surface_destroyed_connection;
    wl_display* m_display { nullptr };
    wl_compositor* m_compositor { nullptr };
    wl_registry* m_registry { nullptr };
    ext_background_effect_manager_v1* m_manager { nullptr };
    ext_background_effect_surface_v1* m_effect { nullptr };
    wl_surface* m_surface { nullptr };
    uint32_t m_manager_name { 0 };
    bool m_blur_supported { false };
    bool m_capabilities_received { false };
    QRegion m_regions;
};

std::unique_ptr<PlatformWindowEffects> create_platform_window_effects(QWindow& window)
{
    return std::make_unique<LinuxWindowEffects>(window);
}

}
