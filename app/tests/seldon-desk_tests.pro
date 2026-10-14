QT += core gui qml quick quickcontrols2 concurrent testlib
CONFIG += c++17 testcase
TARGET = seldon-desk_tests
TEMPLATE = app

DEFINES += QT_DISABLE_DEPRECATED_UP_TO=0x060900 QT_WARN_DEPRECATED_UP_TO=0x060900
# The repository's fixtures, for the tests that read them.
DEFINES += SELDON_FIXTURES=\\\"$$PWD/../../fixtures\\\"

INCLUDEPATH += ../src

HEADERS += ../src/backend.h ../src/theme.h ../src/translator.h
SOURCES += seldon-desk_tests.cpp ../src/backend.cpp ../src/theme.cpp ../src/translator.cpp
RESOURCES += ../src/resources.qrc
