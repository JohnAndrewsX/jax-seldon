// seldon-desk: the Seldon window. Tells you what changed on your machine, who
// changed it, and why, from the index the engine writes. GUI only: the
// commands that need no display are the engine's (`seldon`), which starts
// this window with `seldon open`.

#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QUrl>

#include "backend.h"
#include "theme.h"
#include "translator.h"

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    app.setApplicationName("seldon");
    app.setApplicationDisplayName("Seldon");
    app.setApplicationVersion("0.5.0-dev");

    // Matches the window to seldon.desktop, so the compositor (Wayland app_id)
    // and the launcher pick up the installed icon. The package is `seldon`;
    // this binary is its window, not its command line.
    app.setDesktopFileName("seldon");
    app.setWindowIcon(QIcon::fromTheme("seldon"));

    QQuickStyle::setStyle("Material");

    // Installed before anything is translated, and every qsTr() binding is
    // re-evaluated when the Omarchy language changes.
    Translator translator;
    app.installTranslator(&translator);

    Theme theme;
    // The state index, or SELDON_INDEX's file in read-only dev mode
    // (CONTRACT.md rule 1).
    Backend backend(Backend::fromEnvironment());

    QQmlApplicationEngine engine;
    QObject::connect(&translator, &Translator::changed, &engine, &QQmlEngine::retranslate);
    engine.rootContext()->setContextProperty("translator", &translator);
    engine.rootContext()->setContextProperty("theme", &theme);
    engine.rootContext()->setContextProperty("backend", &backend);
    engine.load(QUrl("qrc:/Main.qml"));
    if (engine.rootObjects().isEmpty())
        return 1;

    return app.exec();
}
