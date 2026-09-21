cmake_minimum_required(VERSION 3.25)

if (ANDROID OR IOS)
    return()
endif()

find_package(Qt6 6.9 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 QuickWidgets Widgets)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

import_rust_crate(MANIFEST_PATH Rust/Cargo.toml CRATE_NAME photon_core)

qt_add_executable(photon
    App/main.cpp
    Bridge/BrowserView.cpp
    Bridge/PhotonApplication.cpp
    Bridge/PhotonWindow.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/EventLoopImplementationQt.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/EventLoopImplementationQtEventTarget.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/InputMethodUtils.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/NativeWindowContainer.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/SelectDropdown.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/StringUtils.cpp
    ${LADYBIRD_SOURCE_DIR}/UI/Qt/WebContentView.cpp
)

qt_add_resources(photon photon_qml
    PREFIX "/Photon"
    FILES
        UI/Main.qml
        UI/components/AddressBar.qml
        UI/components/BrowserSurface.qml
        UI/components/NavigationBar.qml
        UI/components/NavigationButton.qml
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
    Qt::Qml
    Qt::Quick
    Qt::QuickControls2
    Qt::QuickWidgets
    Qt::Widgets
    photon_core
)

if (LINUX AND NOT ANDROID)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(GIO REQUIRED IMPORTED_TARGET gio-2.0 gio-unix-2.0)
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
add_dependencies(photon ${ladybird_helper_processes} ladybird_build_resource_files)
add_custom_target(Photon DEPENDS photon)
