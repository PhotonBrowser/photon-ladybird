# SPDX-License-Identifier: GPL-3.0-only
cmake_minimum_required(VERSION 3.25)

if (ANDROID OR IOS)
    return()
endif()

find_package(Qt6 6.9 REQUIRED COMPONENTS Core Gui Widgets)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

import_rust_crate(MANIFEST_PATH Rust/Cargo.toml CRATE_NAME photon_core)

find_program(PHOTON_NPM_EXECUTABLE npm REQUIRED)
set(PHOTON_WEBUI_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/WebUI")
set(PHOTON_WEBUI_DIST_INDEX "${PHOTON_WEBUI_SOURCE_DIR}/dist/index.html")
file(GLOB_RECURSE PHOTON_WEBUI_SOURCES CONFIGURE_DEPENDS
    "${PHOTON_WEBUI_SOURCE_DIR}/src/*"
)
list(APPEND PHOTON_WEBUI_SOURCES
    "${PHOTON_WEBUI_SOURCE_DIR}/index.html"
    "${PHOTON_WEBUI_SOURCE_DIR}/package.json"
    "${PHOTON_WEBUI_SOURCE_DIR}/package-lock.json"
    "${PHOTON_WEBUI_SOURCE_DIR}/tsconfig.json"
    "${PHOTON_WEBUI_SOURCE_DIR}/tsconfig.app.json"
    "${PHOTON_WEBUI_SOURCE_DIR}/tsconfig.node.json"
    "${PHOTON_WEBUI_SOURCE_DIR}/vite.config.ts"
)
add_custom_command(
    OUTPUT "${PHOTON_WEBUI_DIST_INDEX}"
    COMMAND "${PHOTON_NPM_EXECUTABLE}" run build
    WORKING_DIRECTORY "${PHOTON_WEBUI_SOURCE_DIR}"
    DEPENDS ${PHOTON_WEBUI_SOURCES}
    COMMENT "Building Photon Web UI with Vite"
    VERBATIM
)
add_custom_target(photon_webui_assets DEPENDS "${PHOTON_WEBUI_DIST_INDEX}")
set_source_files_properties("${PHOTON_WEBUI_DIST_INDEX}" PROPERTIES GENERATED TRUE)

qt_add_executable(photon
    App/main.cpp
    Bridge/BrowserView.cpp
    Bridge/ChromeSurface.cpp
    Bridge/PhotonCommandTransport.cpp
    Bridge/PhotonApplication.cpp
    Bridge/PhotonWindow.cpp
    Bridge/WindowEffects.cpp
    Bridge/WindowScene.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/EventLoopImplementationQt.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/EventLoopImplementationQtEventTarget.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/InputMethodUtils.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/NativeWindowContainer.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/SelectDropdown.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/StringUtils.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/WebContentView.cpp
)

qt_add_resources(photon photon_webui
    PREFIX "/Photon"
    FILES
        WebUI/dist/index.html
)

target_include_directories(photon PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/Rust
)

target_link_libraries(photon PRIVATE
    AK
    LibCore
    LibFileSystem
    LibGfx
    LibImageDecoderClient
    LibIPC
    LibJS
    LibMain
    LibRequests
    LibURL
    LibWakeLock
    LibWeb
    LibWebView
    OpenSSL::Crypto
    OpenSSL::SSL
    Qt::Core
    Qt::Gui
    Qt::Widgets
    photon_core
)

if (LINUX AND NOT ANDROID)
    set(QT_NO_PRIVATE_MODULE_WARNING ON)
    find_package(Qt6 6.9 REQUIRED COMPONENTS GuiPrivate)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(GIO REQUIRED IMPORTED_TARGET gio-2.0 gio-unix-2.0)
    pkg_check_modules(WAYLAND_CLIENT REQUIRED IMPORTED_TARGET wayland-client)
    find_program(PHOTON_WAYLAND_SCANNER wayland-scanner REQUIRED)

    set(PHOTON_BACKGROUND_EFFECT_XML "${CMAKE_CURRENT_SOURCE_DIR}/Bridge/Platform/Protocols/ext-background-effect-v1.xml")
    set(PHOTON_BACKGROUND_EFFECT_HEADER "${CMAKE_CURRENT_BINARY_DIR}/ext-background-effect-v1-client-protocol.h")
    set(PHOTON_BACKGROUND_EFFECT_CODE "${CMAKE_CURRENT_BINARY_DIR}/ext-background-effect-v1-protocol.c")
    add_custom_command(
        OUTPUT "${PHOTON_BACKGROUND_EFFECT_HEADER}"
        COMMAND "${PHOTON_WAYLAND_SCANNER}" client-header "${PHOTON_BACKGROUND_EFFECT_XML}" "${PHOTON_BACKGROUND_EFFECT_HEADER}"
        DEPENDS "${PHOTON_BACKGROUND_EFFECT_XML}"
        VERBATIM
    )
    add_custom_command(
        OUTPUT "${PHOTON_BACKGROUND_EFFECT_CODE}"
        COMMAND "${PHOTON_WAYLAND_SCANNER}" private-code "${PHOTON_BACKGROUND_EFFECT_XML}" "${PHOTON_BACKGROUND_EFFECT_CODE}"
        DEPENDS "${PHOTON_BACKGROUND_EFFECT_XML}" "${PHOTON_BACKGROUND_EFFECT_HEADER}"
        VERBATIM
    )
    # The upstream C source inherits C++-only project flags (for example
    # -Wno-dangling-reference) from the shared C/C++ warning policy.
    set_source_files_properties("${PHOTON_BACKGROUND_EFFECT_CODE}" PROPERTIES COMPILE_OPTIONS "-Wno-error")
    target_sources(photon PRIVATE
        Bridge/Platform/LinuxWindowEffects.cpp
        "${PHOTON_BACKGROUND_EFFECT_HEADER}"
        "${PHOTON_BACKGROUND_EFFECT_CODE}"
    )
    target_include_directories(photon PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    target_link_libraries(photon PRIVATE Qt::GuiPrivate PkgConfig::WAYLAND_CLIENT)
else()
    target_sources(photon PRIVATE Bridge/Platform/UnsupportedWindowEffects.cpp)
endif()

if (NOT APPLE AND USE_VULKAN_DMABUF_IMAGES AND Qt6_VERSION VERSION_GREATER_EQUAL 6.7.0)
    set(PHOTON_VERT_SHADER_HEADER "${CMAKE_CURRENT_BINARY_DIR}/WebContentViewLinuxVertShader.h")
    set(PHOTON_FRAG_SHADER_HEADER "${CMAKE_CURRENT_BINARY_DIR}/WebContentViewLinuxFragShader.h")

    add_custom_command(
        OUTPUT "${PHOTON_VERT_SHADER_HEADER}"
        COMMAND ${GLSLANG_VALIDATOR} -V --vn webcontent_vert_shader_spv
            -o "${PHOTON_VERT_SHADER_HEADER}"
            "${LADYBIRD_SOURCE_DIR}/UI/Qt/Shaders/WebContentViewLinux.vert"
        DEPENDS "${LADYBIRD_SOURCE_DIR}/UI/Qt/Shaders/WebContentViewLinux.vert"
        VERBATIM
    )
    add_custom_command(
        OUTPUT "${PHOTON_FRAG_SHADER_HEADER}"
        COMMAND ${GLSLANG_VALIDATOR} -V --vn webcontent_frag_shader_spv
            -o "${PHOTON_FRAG_SHADER_HEADER}"
            "${LADYBIRD_SOURCE_DIR}/UI/Qt/Shaders/WebContentViewLinux.frag"
        DEPENDS "${LADYBIRD_SOURCE_DIR}/UI/Qt/Shaders/WebContentViewLinux.frag"
        VERBATIM
    )

    target_sources(photon PRIVATE
        ${LADYBIRD_SOURCE_DIR}/UI/Qt/WebContentViewLinux.cpp
        "${PHOTON_VERT_SHADER_HEADER}"
        "${PHOTON_FRAG_SHADER_HEADER}"
    )
    target_include_directories(photon PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
    target_link_libraries(photon PRIVATE Qt::GuiPrivate Vulkan::Vulkan)
endif()

if (APPLE)
    target_sources(photon PRIVATE
        ${LADYBIRD_SOURCE_DIR}/UI/AppKit/Utilities/Conversions.mm
        ${LADYBIRD_SOURCE_DIR}/UI/AppKit/Utilities/DictionaryLookup.mm
        ${LADYBIRD_SOURCE_DIR}/UI/Qt/WebContentViewMac.mm
    )
    target_link_libraries(photon PRIVATE Qt::GuiPrivate "-framework Cocoa" "-framework IOSurface" "-framework Metal")
endif()

if (HAS_DIRECTX)
    target_sources(photon PRIVATE ${LADYBIRD_SOURCE_DIR}/UI/Qt/WebContentViewWindows.cpp)
    target_link_libraries(photon PRIVATE Qt::GuiPrivate d3d11)
endif()

set_target_properties(photon PROPERTIES OUTPUT_NAME Photon)
add_dependencies(photon photon_webui_assets ${ladybird_helper_processes} ladybird_build_resource_files)
add_custom_target(Photon DEPENDS photon)
