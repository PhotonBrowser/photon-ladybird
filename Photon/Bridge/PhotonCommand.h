/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#pragma once

#include <QList>
#include <QString>

#include <cstdint>
#include <variant>

namespace Photon {

struct NavigateCommand {
    QString url;
};
struct BackCommand { };
struct ForwardCommand { };
struct ReloadCommand { };
struct FocusAddressCommand { };
struct UiReadyCommand { };
struct NewTabCommand { };
struct OpenSettingsCommand { };
struct SelectTabCommand {
    uint64_t tab_id;
};
struct CloseTabCommand {
    uint64_t tab_id;
};
struct ReorderTabsCommand {
    QList<uint64_t> tab_ids;
};

enum class ThemeMode {
    System,
    Light,
    Dark,
};

struct SetThemeCommand {
    ThemeMode mode;
};
struct SetForceDarkPagesCommand {
    bool enabled;
};
struct SetDimOverlaysCommand {
    bool enabled;
};
struct SetWindowTintOpacityCommand {
    uint8_t opacity;
};
struct SetTitlebarDragRegionCommand {
    bool enabled;
};

enum class OverlayRegion {
    BrowserMenu,
    SiteInfo,
};

struct SetOverlayCaptureCommand {
    OverlayRegion region;
    bool open;
};

enum class WindowCommand {
    Minimize,
    Maximize,
    ToggleMaximize,
    Close,
};

struct WindowControlCommand {
    WindowCommand command;
};

using PhotonCommand = std::variant<
    NavigateCommand,
    BackCommand,
    ForwardCommand,
    ReloadCommand,
    FocusAddressCommand,
    UiReadyCommand,
    NewTabCommand,
    OpenSettingsCommand,
    SelectTabCommand,
    CloseTabCommand,
    ReorderTabsCommand,
    SetThemeCommand,
    SetForceDarkPagesCommand,
    SetDimOverlaysCommand,
    SetWindowTintOpacityCommand,
    SetTitlebarDragRegionCommand,
    SetOverlayCaptureCommand,
    WindowControlCommand>;

}
