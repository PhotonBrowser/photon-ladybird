/*
 * Copyright (c) 2026, the Photon developers.
 *
 * SPDX-License-Identifier: GPL-3.0-only
 */

#pragma once

#include <Photon/Bridge/PhotonCommand.h>

#include <LibURL/URL.h>

#include <optional>

namespace Photon {

// Temporary adapter from the trusted chrome's navigation requests to typed commands.
class PhotonCommandTransport {
public:
    bool handles(URL::URL const&) const;
    std::optional<PhotonCommand> decode(URL::URL const&) const;
};

}
