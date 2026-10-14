# seldon-desk: the Seldon window. The engine (`seldon`, Rust) stays the only
# writer; this app reads the index and runs `seldon … --json` like the plugin.
QT += core gui qml quick quickcontrols2 concurrent

CONFIG += c++17 release
TARGET = seldon-desk
TEMPLATE = app

# Qt 6.9 is the oldest Qt the app builds against (bin/build checks it), so
# API deprecated or added after it stays out of reach.
DEFINES += QT_DISABLE_DEPRECATED_UP_TO=0x060900 QT_WARN_DEPRECATED_UP_TO=0x060900

HEADERS += \
    src/backend.h \
    src/theme.h \
    src/translator.h

SOURCES += \
    src/main.cpp \
    src/backend.cpp \
    src/theme.cpp \
    src/translator.cpp

RESOURCES += src/resources.qrc
