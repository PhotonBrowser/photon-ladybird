/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#pragma once

#include <Photon/Bridge/PhotonCommand.h>

#include <QByteArray>
#include <QString>
#include <optional>

namespace Photon {

// Validates structured messages from the trusted chrome against Photon commands.
class PhotonCommandTransport {
public:
    std::optional<PhotonCommand> decode_message(QString const& type, QByteArray const& payload) const;
};

}
