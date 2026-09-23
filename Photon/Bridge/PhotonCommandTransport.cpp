/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#include <Photon/Bridge/PhotonCommandTransport.h>

#include <UI/Qt/StringUtils.h>

#include <QRegularExpression>
#include <QUrl>
#include <QUrlQuery>

namespace Photon {

bool PhotonCommandTransport::handles(URL::URL const& url) const
{
    return url.scheme() == "photon-command";
}

static bool is_valid_tab_id(QString const& value, uint64_t& tab_id)
{
    static QRegularExpression const pattern(QStringLiteral("^tab-([1-9][0-9]*)$"));
    auto match = pattern.match(value);
    if (!match.hasMatch())
        return false;
    bool converted = false;
    tab_id = match.captured(1).toULongLong(&converted);
    return converted && tab_id != 0;
}

std::optional<PhotonCommand> PhotonCommandTransport::decode(URL::URL const& url) const
{
    if (!handles(url))
        return { };

    QUrl parsed(qstring_from_ak_string(url.serialize()));
    if (!parsed.isValid() || !parsed.userInfo().isEmpty() || parsed.port(-1) != -1 || !parsed.path().isEmpty() || parsed.hasFragment())
        return { };

    auto const items = QUrlQuery(parsed).queryItems(QUrl::FullyDecoded);
    auto const command = parsed.host();
    auto no_arguments = [&]() -> bool { return !parsed.hasQuery() && items.isEmpty(); };
    if (command == QStringLiteral("back") && no_arguments())
        return BackCommand { };
    if (command == QStringLiteral("forward") && no_arguments())
        return ForwardCommand { };
    if (command == QStringLiteral("reload") && no_arguments())
        return ReloadCommand { };
    if (command == QStringLiteral("new-tab") && no_arguments())
        return NewTabCommand { };
    if (command == QStringLiteral("open-settings") && no_arguments())
        return OpenSettingsCommand { };

    if (command == QStringLiteral("window-minimize") && no_arguments())
        return WindowControlCommand { WindowCommand::Minimize };
    if (command == QStringLiteral("window-toggle-maximize") && no_arguments())
        return WindowControlCommand { WindowCommand::ToggleMaximize };
    if (command == QStringLiteral("window-close") && no_arguments())
        return WindowControlCommand { WindowCommand::Close };
    if ((command == QStringLiteral("window-drag") || command == QStringLiteral("window-start-system-move")) && no_arguments())
        return WindowControlCommand { WindowCommand::StartSystemMove };

    if (items.size() != 1 || items.first().first != QStringLiteral("value"))
        return { };
    auto const& value = items.first().second;

    if (command == QStringLiteral("navigate")) {
        if (!value.trimmed().isEmpty())
            return NavigateCommand { value };
        return { };
    }

    uint64_t tab_id = 0;
    if (command == QStringLiteral("select-tab") && is_valid_tab_id(value, tab_id))
        return SelectTabCommand { tab_id };
    if (command == QStringLiteral("close-tab") && is_valid_tab_id(value, tab_id))
        return CloseTabCommand { tab_id };

    if (command == QStringLiteral("reorder-tabs")) {
        ReorderTabsCommand reorder;
        for (auto const& item : value.split(QLatin1Char(','), Qt::KeepEmptyParts)) {
            if (!is_valid_tab_id(item, tab_id))
                return { };
            reorder.tab_ids.append(tab_id);
        }
        if (!reorder.tab_ids.isEmpty())
            return reorder;
        return { };
    }

    if (command == QStringLiteral("set-theme")) {
        if (value == QStringLiteral("system"))
            return SetThemeCommand { ThemeMode::System };
        if (value == QStringLiteral("light"))
            return SetThemeCommand { ThemeMode::Light };
        if (value == QStringLiteral("dark"))
            return SetThemeCommand { ThemeMode::Dark };
        return { };
    }

    if (command == QStringLiteral("set-dim-overlays")) {
        if (value == QStringLiteral("true"))
            return SetDimOverlaysCommand { true };
        if (value == QStringLiteral("false"))
            return SetDimOverlaysCommand { false };
        return { };
    }

    if (command == QStringLiteral("set-force-dark-pages")) {
        if (value == QStringLiteral("true"))
            return SetForceDarkPagesCommand { true };
        if (value == QStringLiteral("false"))
            return SetForceDarkPagesCommand { false };
        return { };
    }

    if (command == QStringLiteral("capture")) {
        auto parts = value.split(QLatin1Char(':'), Qt::KeepEmptyParts);
        if (parts.size() != 2 || (parts[1] != QStringLiteral("open") && parts[1] != QStringLiteral("close")))
            return { };
        if (parts[0] == QStringLiteral("browser-menu"))
            return SetOverlayCaptureCommand { OverlayRegion::BrowserMenu, parts[1] == QStringLiteral("open") };
        if (parts[0] == QStringLiteral("site-info"))
            return SetOverlayCaptureCommand { OverlayRegion::SiteInfo, parts[1] == QStringLiteral("open") };
    }

    return { };
}

}
