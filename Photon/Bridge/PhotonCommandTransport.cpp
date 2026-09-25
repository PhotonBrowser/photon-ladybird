/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/PhotonCommandTransport.h>

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

#include <algorithm>

namespace Photon {

static bool is_valid_tab_id(QString const& value, uint64_t& tab_id)
{
    if (!value.startsWith(QStringLiteral("tab-")))
        return false;

    auto suffix = value.sliced(4);
    if (suffix.isEmpty() || suffix.startsWith(QLatin1Char('0')))
        return false;
    for (auto character : suffix) {
        if (!character.isDigit() || character.unicode() > 0x7f)
            return false;
    }

    bool converted = false;
    tab_id = suffix.toULongLong(&converted);
    return converted && tab_id != 0;
}

std::optional<PhotonCommand> PhotonCommandTransport::decode_message(QString const& type, QByteArray const& payload) const
{
    if (type.isEmpty() || type.size() > 128 || payload.size() > 64 * 1024)
        return { };

    QJsonParseError error;
    auto const document = QJsonDocument::fromJson(payload, &error);
    if (error.error != QJsonParseError::NoError || !document.isObject())
        return { };
    auto const object = document.object();
    auto has_exact_fields = [&](std::initializer_list<QStringView> fields) {
        if (object.size() != static_cast<int>(fields.size()))
            return false;
        return std::all_of(fields.begin(), fields.end(), [&](QStringView field) { return object.contains(field); });
    };
    auto empty_payload = [&] { return object.isEmpty(); };

    if (type == QStringLiteral("back") && empty_payload())
        return BackCommand { };
    if (type == QStringLiteral("forward") && empty_payload())
        return ForwardCommand { };
    if (type == QStringLiteral("reload") && empty_payload())
        return ReloadCommand { };
    if (type == QStringLiteral("focus-address") && empty_payload())
        return FocusAddressCommand { };
    if (type == QStringLiteral("new-tab") && empty_payload())
        return NewTabCommand { };
    if (type == QStringLiteral("open-settings") && empty_payload())
        return OpenSettingsCommand { };
    if (type == QStringLiteral("window-minimize") && empty_payload())
        return WindowControlCommand { WindowCommand::Minimize };
    if (type == QStringLiteral("window-maximize") && empty_payload())
        return WindowControlCommand { WindowCommand::Maximize };
    if (type == QStringLiteral("window-toggle-maximize") && empty_payload())
        return WindowControlCommand { WindowCommand::ToggleMaximize };
    if (type == QStringLiteral("window-close") && empty_payload())
        return WindowControlCommand { WindowCommand::Close };
    if (type == QStringLiteral("window-start-system-move") && empty_payload())
        return WindowControlCommand { WindowCommand::StartSystemMove };

    if (type == QStringLiteral("navigate") && has_exact_fields({ u"url" })) {
        auto const value = object.value(QStringLiteral("url"));
        if (value.isString() && !value.toString().trimmed().isEmpty() && value.toString().size() <= 8192)
            return NavigateCommand { value.toString() };
        return { };
    }

    uint64_t tab_id = 0;
    if ((type == QStringLiteral("select-tab") || type == QStringLiteral("close-tab")) && has_exact_fields({ u"tabId" })) {
        auto const value = object.value(QStringLiteral("tabId"));
        if (!value.isString() || !is_valid_tab_id(value.toString(), tab_id))
            return { };
        return type == QStringLiteral("select-tab") ? PhotonCommand { SelectTabCommand { tab_id } } : PhotonCommand { CloseTabCommand { tab_id } };
    }

    if (type == QStringLiteral("reorder-tabs") && has_exact_fields({ u"tabIds" })) {
        auto const value = object.value(QStringLiteral("tabIds"));
        if (!value.isArray() || value.toArray().isEmpty() || value.toArray().size() > 256)
            return { };
        ReorderTabsCommand command;
        for (auto const& item : value.toArray()) {
            if (!item.isString() || !is_valid_tab_id(item.toString(), tab_id))
                return { };
            command.tab_ids.append(tab_id);
        }
        return command;
    }

    if (type == QStringLiteral("set-theme") && has_exact_fields({ u"mode" })) {
        auto const value = object.value(QStringLiteral("mode"));
        if (!value.isString())
            return { };
        if (value.toString() == QStringLiteral("system"))
            return SetThemeCommand { ThemeMode::System };
        if (value.toString() == QStringLiteral("light"))
            return SetThemeCommand { ThemeMode::Light };
        if (value.toString() == QStringLiteral("dark"))
            return SetThemeCommand { ThemeMode::Dark };
        return { };
    }

    if ((type == QStringLiteral("set-force-dark-pages") || type == QStringLiteral("set-dim-overlays")) && has_exact_fields({ u"enabled" })) {
        auto const value = object.value(QStringLiteral("enabled"));
        if (!value.isBool())
            return { };
        if (type == QStringLiteral("set-force-dark-pages"))
            return SetForceDarkPagesCommand { value.toBool() };
        return SetDimOverlaysCommand { value.toBool() };
    }

    if (type == QStringLiteral("capture") && has_exact_fields({ u"region", u"open" })) {
        auto const region = object.value(QStringLiteral("region"));
        auto const open = object.value(QStringLiteral("open"));
        if (!region.isString() || !open.isBool())
            return { };
        if (region.toString() == QStringLiteral("browser-menu"))
            return SetOverlayCaptureCommand { OverlayRegion::BrowserMenu, open.toBool() };
        if (region.toString() == QStringLiteral("site-info"))
            return SetOverlayCaptureCommand { OverlayRegion::SiteInfo, open.toBool() };
    }

    return { };
}

}
