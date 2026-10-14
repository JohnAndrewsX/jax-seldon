// Theme and translator cases follow the omarchy-app skill's template (MIT,
// David Heinemeier Hansson). Everything that reads the disk gets a temp
// directory: the Theme its theme directory, the Translator its language file,
// the Backend its index path and a fake engine. Nothing here reads the real
// home, the real Seldon state or the real runtime directory.
#include <QDir>
#include <QFile>
#include <QGuiApplication>
#include <QJSEngine>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QRegularExpression>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtTest>

#include "backend.h"
#include "theme.h"
#include "translator.h"

namespace {

QStringList messages;

// Every warning the QML engine prints is a failure of the window test.
void collect(QtMsgType type, const QMessageLogContext &, const QString &message) {
    if (type != QtDebugMsg && type != QtInfoMsg)
        messages.append(message);
    fprintf(stderr, "%s\n", qPrintable(message));
}

bool write(const QString &path, const QByteArray &contents) {
    QFile file(path);
    return file.open(QIODevice::WriteOnly) && file.write(contents) == contents.size();
}

QByteArray read(const QString &path) {
    QFile file(path);
    return file.open(QIODevice::ReadOnly) ? file.readAll() : QByteArray();
}

const QString Fixture = QStringLiteral(SELDON_FIXTURES "/index.sample.json");

// An engine that logs its arguments and answers the calls the window makes.
// `status-exit` in its directory sets the exit code of `status`.
QString fakeEngine(const QTemporaryDir &dir) {
    const QString path = dir.filePath("seldon");
    const QByteArray d = dir.path().toUtf8();
    const QByteArray script =
        "#!/bin/sh\n"
        "printf '%s\\n' \"$*\" >> '" + d + "/calls'\n"
        "case \"$1\" in\n"
        "  --version) echo '{\"version\":\"0.4.0\"}' ;;\n"
        "  status) exit \"$(cat '" + d + "/status-exit' 2>/dev/null || echo 0)\" ;;\n"
        "  capture) echo '{\"written\":0,\"collectors\":[]}' ;;\n"
        "  drift) echo '{\"resolved\":1,\"events\":[],\"case\":{\"id\":\"C-2026-005\"}}' ;;\n"
        "esac\n";
    write(path, script);
    QFile::setPermissions(path, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    return path;
}

QStringList calls(const QTemporaryDir &dir) {
    return QString::fromUtf8(read(dir.filePath("calls"))).split('\n', Qt::SkipEmptyParts);
}

Backend::Options options(const QTemporaryDir &dir, const QString &index, bool devMode = false) {
    Backend::Options o;
    o.indexPath = index;
    o.engine = fakeEngine(dir);
    o.launcher = dir.filePath("launcher");
    o.devMode = devMode;
    return o;
}

// A constant of Model.js, as the plugin evaluates it.
QJSValue modelJs(const char *name) {
    static QJSEngine js;
    static const bool loaded = !js.evaluate(QString::fromUtf8(read(":/Model.js")), "Model.js").isError();
    return loaded ? js.globalObject().property(name) : QJSValue();
}

// The fixture with `edit` applied, written to `path`.
template <typename Edit> bool writeFixture(const QString &path, Edit edit) {
    QJsonObject index = QJsonDocument::fromJson(read(Fixture)).object();
    edit(index);
    return write(path, QJsonDocument(index).toJson(QJsonDocument::Compact));
}

// The fixture's open cases without the first one.
void dropFirstCase(QJsonObject &index) {
    QJsonObject cases = index["cases"].toObject();
    QJsonArray active = cases["active"].toArray();
    active.removeAt(0);
    cases["active"] = active;
    index["cases"] = cases;
}

QByteArray indexWith(const QString &generatedAt, const QString &state = "ok", int contract = 2) {
    QJsonObject index{{"contractVersion", contract},
                      {"generatedAt", generatedAt},
                      {"summary", QJsonObject{{"openDrift", 0}, {"crisis", 0}}},
                      {"state", QJsonObject{{"status", state}}}};
    return QJsonDocument(index).toJson(QJsonDocument::Compact);
}

} // namespace

class SeldonDeskTests : public QObject {
    Q_OBJECT

private slots:
    // ---- Theme ----------------------------------------------------------

    void themeFallsBack() {
        QTemporaryDir dir;
        Theme theme(dir.path());
        QCOMPARE(theme.accent(), QString("#FFD60A"));
        QCOMPARE(theme.accentForeground(), QString("black"));
        QCOMPARE(theme.background(), QString("#101315"));
        QCOMPARE(theme.radius(), 0);
    }

    void themeReadsAccent() {
        QTemporaryDir dir;
        QDir(dir.path()).mkpath("theme");
        QVERIFY(write(dir.path() + "/theme/colors.toml", "accent = \"#112233\"\n"));
        Theme theme(dir.path());
        QCOMPARE(theme.accent(), QString("#112233"));
        QCOMPARE(theme.accentForeground(), QString("white"));
    }

    // The shell's roles: red or color1 (the last) for urgent, color8 then the
    // foreground for muted, the fills over the foreground.
    void themeReadsThePalette() {
        QTemporaryDir dir;
        QDir(dir.path()).mkpath("theme");
        QVERIFY(write(dir.path() + "/theme/colors.toml",
                      "background = \"#1a1b26\"\nforeground = \"#a9b1d6\"\ncolor4 = \"#7aa2f7\"\n"
                      "color1 = \"#f7768e\"\n"));
        Theme theme(dir.path());
        QCOMPARE(theme.background(), QString("#1a1b26"));
        QCOMPARE(theme.accent(), QString("#7aa2f7"));
        QCOMPARE(theme.urgent(), QString("#f7768e"));
        QCOMPARE(theme.muted(), QString("#a9b1d6"));
        QCOMPARE(theme.normalFill(), QString("#0aa9b1d6"));
        QCOMPARE(theme.selectedFill(), QString("#2ea9b1d6"));
    }

    void themeFollowsAThemeSwitch() {
        QTemporaryDir dir;
        QDir(dir.path()).mkpath("theme");
        QVERIFY(write(dir.path() + "/theme/colors.toml", "accent = \"#112233\"\n"));
        Theme theme(dir.path());
        // A switch replaces the file, as Omarchy's does.
        QVERIFY(write(dir.path() + "/theme/colors.next", "accent = \"#445566\"\n"));
        QVERIFY(QFile::remove(dir.path() + "/theme/colors.toml"));
        QVERIFY(QFile::rename(dir.path() + "/theme/colors.next", dir.path() + "/theme/colors.toml"));
        QTRY_COMPARE(theme.accent(), QString("#445566"));
    }

    // The shell reads the theme under the home, not under XDG_STATE_HOME
    // (Commons/Color.qml), and so does the window.
    void themeIgnoresXdgStateHome() {
        QTemporaryDir home, state;
        QDir(home.path()).mkpath(".local/state/omarchy/current/theme");
        QDir(state.path()).mkpath("omarchy/current/theme");
        QVERIFY(write(home.path() + "/.local/state/omarchy/current/theme/colors.toml", "accent = \"#112233\"\n"));
        QVERIFY(write(state.path() + "/omarchy/current/theme/colors.toml", "accent = \"#445566\"\n"));
        const QByteArray oldHome = qgetenv("HOME"), oldState = qgetenv("XDG_STATE_HOME"),
                         oldHypr = qgetenv("HYPRLAND_INSTANCE_SIGNATURE");
        qputenv("HOME", home.path().toUtf8());
        qputenv("XDG_STATE_HOME", state.path().toUtf8());
        qunsetenv("HYPRLAND_INSTANCE_SIGNATURE");
        const QString accent = Theme().accent();
        qputenv("HOME", oldHome);
        qputenv("XDG_STATE_HOME", oldState);
        if (!oldHypr.isEmpty())
            qputenv("HYPRLAND_INSTANCE_SIGNATURE", oldHypr);
        QCOMPARE(accent, QString("#112233"));
    }

    void themeFollowsHyprlandRounding() {
        QTemporaryDir dir;
        const QString response = dir.filePath("response.json"), command = dir.filePath("hyprctl");
        QVERIFY(write(command, "#!/bin/sh\ncat '" + response.toUtf8() + "'\n"));
        QVERIFY(QFile::setPermissions(command, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner));
        QVERIFY(write(response, "{\"int\":0}"));

        Theme theme(dir.path(), nullptr, command);
        QCOMPARE(theme.radius(), 0);
        QVERIFY(write(response, "{\"int\":6}"));
        QTRY_COMPARE(theme.radius(), 6);
    }

    // ---- Translator -----------------------------------------------------

    void translatorFallsBackToEnglish() {
        QTemporaryDir dir;
        Translator translator(dir.path() + "/language", dir.path());
        QCOMPARE(translator.language(), QString("en"));
        QVERIFY(translator.translate("Main", "Save").isNull());
    }

    void translatorFollowsTheLanguageFile() {
        QTemporaryDir dir;
        QVERIFY(write(dir.path() + "/language", "pt-BR\n"));
        QVERIFY(write(dir.path() + "/pt.json", R"({"Save": "Guardar", "Open": "Abrir"})"));
        QVERIFY(write(dir.path() + "/pt-BR.json", R"({"Save": "Salvar"})"));

        Translator translator(dir.path() + "/language", dir.path());
        QCOMPARE(translator.language(), QString("pt-BR"));
        QCOMPARE(translator.translate("Main", "Save"), QString("Salvar"));
        QCOMPARE(translator.translate("Main", "Open"), QString("Abrir"));
        QVERIFY(translator.translate("Main", "Quit").isNull());
    }

    void translatorPicksPluralForms() {
        QTemporaryDir dir;
        QVERIFY(write(dir.path() + "/language", "ru\n"));
        QVERIFY(write(dir.path() + "/en.json", R"({"%n clips": {"one": "%n clip", "other": "%n clips"}})"));
        QVERIFY(write(dir.path() + "/ru.json",
                      R"({"%n clips": {"one": "%n клип", "few": "%n клипа", "many": "%n клипов", "other": "%n клипа"}})"));

        Translator translator(dir.path() + "/language", dir.path());
        QCOMPARE(translator.translate("Main", "%n clips", nullptr, 21), QString("%n клип"));
        QCOMPARE(translator.translate("Main", "%n clips", nullptr, 22), QString("%n клипа"));
        QCOMPARE(translator.translate("Main", "%n clips", nullptr, 25), QString("%n клипов"));
        QCOMPARE(Translator::pluralCategory("ar", 11), QString("many"));
        QCOMPARE(Translator::pluralCategory("ja", 1), QString("other"));
    }

    // Every literal qsTr() of the window has its German, and the catalogs
    // are flat objects of non-empty strings.
    void germanCoversEveryString() {
        const QJsonObject german = QJsonDocument::fromJson(read(":/i18n/de.json")).object();
        QVERIFY(!german.isEmpty());
        QVERIFY(QJsonDocument::fromJson(read(":/i18n/en.json")).isObject());
        const QRegularExpression literal(QStringLiteral("qsTr\\(\"((?:[^\"\\\\]|\\\\.)*)\"\\)"));
        QStringList missing;
        for (const QString &file : QDir(":/").entryList({"*.qml"})) {
            const QString source = QString::fromUtf8(read(":/" + file));
            for (auto it = literal.globalMatch(source); it.hasNext();) {
                const QString key = it.next().captured(1).replace("\\\"", "\"");
                if (german.value(key).toString().isEmpty())
                    missing << file + ": " + key;
            }
        }
        QVERIFY2(missing.isEmpty(), qPrintable(missing.join('\n')));
    }

    // ---- Backend: the index ---------------------------------------------

    void parsesTheFixture() {
        const auto parsed = Backend::readIndex(Fixture);
        QVERIFY2(parsed.error.isEmpty(), qPrintable(parsed.error + " " + parsed.detail));
        QCOMPARE(parsed.contractVersion, 2);
        QVERIFY(parsed.index.value("events").isArray());
    }

    void refusesWhatIsNoIndex() {
        QCOMPARE(Backend::parseIndex("").error, QString("empty"));
        QCOMPARE(Backend::parseIndex("  \n").error, QString("empty"));
        QCOMPARE(Backend::parseIndex("{").error, QString("parse"));
        QCOMPARE(Backend::parseIndex("[]").error, QString("shape"));
        QCOMPARE(Backend::parseIndex(R"({"contractVersion":2})").error, QString("shape"));
        QCOMPARE(Backend::parseIndex(indexWith("2026-10-01T17:05:12+02:00", "ok", 1)).error, QString("contract"));
        QCOMPARE(Backend::readIndex(QDir::temp().filePath("seldon-desk-no-such/index.json")).error, QString("missing"));
    }

    // CONTRACT.md rule 3 (ADR-0051): a newer index is read when it says this
    // contract can read it, refused otherwise.
    void readsANewerContractThatSaysSo() {
        QJsonObject index = QJsonDocument::fromJson(indexWith("2026-10-01T17:05:12+02:00", "ok", 3)).object();
        QCOMPARE(Backend::parseIndex(QJsonDocument(index).toJson()).error, QString("contract"));
        index["contractReadableFrom"] = 2;
        const auto parsed = Backend::parseIndex(QJsonDocument(index).toJson());
        QVERIFY(parsed.error.isEmpty());
        QVERIFY(parsed.newer);
        index["contractReadableFrom"] = 3;
        QCOMPARE(Backend::parseIndex(QJsonDocument(index).toJson()).error, QString("contract"));
    }

    // CONTRACT.md rule 5: 1 000 000 bytes or more is refused unread.
    void refusesAnIndexOverTheBudget() {
        QTemporaryDir dir;
        const QString path = dir.filePath("index.json");
        QByteArray big = indexWith("2026-10-01T17:05:12+02:00");
        big.append(QByteArray(Backend::IndexMaxBytes - big.size(), ' '));
        QVERIFY(write(path, big));
        const auto parsed = Backend::readIndex(path);
        QCOMPARE(parsed.error, QString("size"));
        QCOMPARE(parsed.size, Backend::IndexMaxBytes);
        big.chop(1);
        QVERIFY(write(path, big));
        QVERIFY(Backend::readIndex(path).error.isEmpty());
    }

    // The app's constants are Model.js's: the plugin and the window read the
    // same contract, budget and age.
    void constantsMatchModelJs() {
        QJSEngine js;
        const QJSValue result = js.evaluate(QString::fromUtf8(read(":/Model.js")), "Model.js");
        QVERIFY2(!result.isError(), qPrintable(result.toString()));
        QCOMPARE(js.globalObject().property("CONTRACT_VERSION").toInt(), Backend::ContractVersion);
        QCOMPARE(qint64(js.globalObject().property("INDEX_MAX_CHARS").toNumber()), Backend::IndexMaxBytes);
        QCOMPARE(qint64(js.globalObject().property("STALE_AFTER_MS").toNumber()), Backend::StaleAfterMs);
        QCOMPARE(js.globalObject().property("STATUSES").property(0).toString(), QString("ok"));
    }

    // Model.deriveStatus's precedence.
    void derivesTheStatus() {
        const auto ok = Backend::parseIndex(indexWith("2026-10-01T17:05:12+02:00"));
        const qint64 generated = QDateTime::fromString("2026-10-01T17:05:12+02:00", Qt::ISODate).toMSecsSinceEpoch();
        const qint64 hour = 60 * 60 * 1000;
        QCOMPARE(Backend::deriveStatus("missing", "loaded", ok, false, generated), QString("engineMissing"));
        QCOMPARE(Backend::deriveStatus("unknown", "loading", {}, false, generated), QString("loading"));
        QCOMPARE(Backend::deriveStatus("present", "missing", {}, false, generated), QString("indexMissing"));
        QCOMPARE(Backend::deriveStatus("present", "missing", {}, true, generated), QString("notInitialised"));
        QCOMPARE(Backend::deriveStatus("present", "loaded", ok, false, generated + hour), QString("ok"));
        QCOMPARE(Backend::deriveStatus("unknown", "loaded", ok, false, generated + hour), QString("ok"));
        QCOMPARE(Backend::deriveStatus("present", "loaded", ok, false, generated + 3 * hour), QString("indexStale"));
        QCOMPARE(Backend::deriveStatus("present", "loaded", ok, true, generated), QString("notInitialised"));
        const auto notInit = Backend::parseIndex(indexWith("2026-10-01T17:05:12+02:00", "notInitialised"));
        QCOMPARE(Backend::deriveStatus("present", "loaded", notInit, false, generated), QString("notInitialised"));
        const auto old = Backend::parseIndex(indexWith("2026-10-01T17:05:12+02:00", "ok", 1));
        QCOMPARE(Backend::deriveStatus("present", "loaded", old, false, generated), QString("contractMismatch"));
        const auto broken = Backend::parseIndex("{");
        QCOMPARE(Backend::deriveStatus("present", "invalid", broken, false, generated), QString("indexMissing"));
    }

    // ---- Backend: engine calls ------------------------------------------

    void allowsOnlyTheContractForms() {
        const QString event = "01M3VTGNY0NZG4AY80814WSKGR";
        const QList<QStringList> allowed{
            {"--version", "--json"},
            {"status", "--json"},
            {"capture", "--all", "--json", "--quiet"},
            {"drift", "show", event, "--json"},
            {"drift", "link", event, "C-2026-005", "--json"},
            {"drift", "link", event, "C-2026-005", "--only", "--json"},
            {"drift", "dismiss", event, "--json", "--", "not mine"},
            {"drift", "dismiss", event, "--only", "--json", "--", "-rf; rm"},
            {"drift", "explain", event, "--json", "--", "tried a theme"},
            {"drift", "explain", event, "--only", "--zone", "red", "--risk", "R2", "--area", "hypr-land", "--json", "--", "x"},
        };
        for (const auto &args : allowed)
            QVERIFY2(Backend::validateArgs(args).isEmpty(), qPrintable(args.join(' ') + ": " + Backend::validateArgs(args)));
        const QList<QStringList> refused{
            {},
            {"--", "text"},
            {"status"},
            {"status", "--json", "--json"},
            {"capture", "--all", "--json"},
            {"init", "--defaults", "--json"},
            {"open", "journal", "--editor"},
            {"drift", "link", "not-an-id", "C-2026-005", "--json"},
            {"drift", "link", event, "C-26-5", "--json"},
            {"drift", "link", event, "C-2026-005", "--json", "--", "text"},
            {"drift", "dismiss", event, "--json"},
            {"drift", "dismiss", event, "--json", "--", "   "},
            {"drift", "dismiss", event, "--json", "--", "two\nlines"},
            {"drift", "dismiss", event, "--json", "--", "a", "b"},
            {"drift", "explain", event, "--zone", "purple", "--json", "--", "x"},
            {"drift", "explain", event, "--risk", "R1", "--zone", "red", "--json", "--", "x"},
            {"drift", "explain", event, "--area", "Bad Area", "--json", "--", "x"},
            {"drift", "propose", event, "--json"},
            {"drift", "show", event, "--json", "--", "x"},
            {"status", QString("--js") + QChar(0) + "on"},
        };
        for (const auto &args : refused)
            QVERIFY2(!Backend::validateArgs(args).isEmpty(), qPrintable(args.join(' ')));
    }

    void findsTheEngineAndTheIndex() {
        QTemporaryDir dir;
        const QString path = dir.filePath("state/seldon/index.json");
        Backend backend(options(dir, path));
        QTRY_COMPARE(backend.engineState(), QString("present"));
        QCOMPARE(backend.engineVersion(), QString("0.4.0"));
        // No index: the window asks `status` once, which says whether a
        // logbook exists (this fake writes nothing).
        QTRY_COMPARE(backend.status(), QString("indexMissing"));
        QTRY_VERIFY(calls(dir).contains("status --json"));

        // The engine creates the directory and renames the index into it.
        QVERIFY(QDir(dir.path()).mkpath("state/seldon"));
        const QString now = QDateTime::currentDateTimeUtc().toString(Qt::ISODate);
        QVERIFY(write(path + ".tmp", indexWith(now)));
        QVERIFY(QFile::rename(path + ".tmp", path));
        QTRY_COMPARE(backend.status(), QString("ok"));
        QVERIFY(backend.hasIndex());

        // A second rename: the watch re-armed after the first.
        QSignalSpy changed(&backend, &Backend::indexChanged);
        QVERIFY(write(path + ".tmp", indexWith(now, "notInitialised")));
        QVERIFY(QFile::remove(path));
        QVERIFY(QFile::rename(path + ".tmp", path));
        QTRY_COMPARE(backend.status(), QString("notInitialised"));
        QVERIFY(changed.count() >= 1);
    }

    void saysWhenTheEngineIsMissing() {
        QTemporaryDir dir;
        auto o = options(dir, Fixture, true);
        o.engine = dir.filePath("no-such-seldon");
        Backend backend(o);
        QTRY_COMPARE(backend.status(), QString("engineMissing"));
        QVERIFY(!backend.run({"status", "--json"}));
    }

    // A `seldon` that does not answer its version in time is no engine.
    void aHangingEngineCountsAsMissing() {
        QTemporaryDir dir;
        auto o = options(dir, Fixture, true);
        QVERIFY(write(o.engine, "#!/bin/sh\nexec sleep 30\n"));
        o.probeTimeoutMs = 300;
        Backend backend(o);
        QTRY_COMPARE(backend.status(), QString("engineMissing"));
    }

    // Exit 3 from `status` is a logbook not initialised yet; an index written
    // after it ends that.
    void notInitialisedFromExitThree() {
        QTemporaryDir dir;
        QVERIFY(write(dir.filePath("status-exit"), "3\n"));
        Backend backend(options(dir, dir.filePath("index.json")));
        QTRY_COMPARE(backend.status(), QString("notInitialised"));
        QVERIFY(!backend.fix());

        QVERIFY(write(dir.filePath("status-exit"), "0\n"));
        QTest::qWait(1100);
        const QString now = QDateTime::currentDateTimeUtc().toString(Qt::ISODate);
        QVERIFY(write(dir.filePath("index.json"), indexWith(now)));
        QTRY_COMPARE(backend.status(), QString("ok"));
    }

    // The fixes the engine does itself: build (status) and capture.
    void fixesRunTheEngine() {
        QTemporaryDir dir;
        QVERIFY(write(dir.filePath("index.json"), indexWith("2026-10-01T17:05:12+02:00")));
        Backend backend(options(dir, dir.filePath("index.json")));
        QTRY_COMPARE(backend.status(), QString("indexStale"));
        QTRY_COMPARE(backend.engineState(), QString("present"));
        QVERIFY(backend.fix());
        QTRY_VERIFY(!backend.busy());
        QCOMPARE(calls(dir).mid(1), (QStringList{"capture --all --json --quiet", "status --json"}));
    }

    // CONTRACT.md rule 1: dev mode renders the file, probes the version and
    // runs nothing else; its clock is the index's.
    void devModeRunsNothing() {
        QTemporaryDir dir;
        Backend backend(options(dir, Fixture, true));
        QTRY_COMPARE(backend.engineState(), QString("present"));
        QTRY_COMPARE(backend.status(), QString("ok"));
        QVERIFY(!backend.run({"status", "--json"}));
        QVERIFY(!backend.fix());
        QVERIFY(!backend.openTerminal({Backend::TerminalLauncher, "echo hi"}));
        QCOMPARE(calls(dir), QStringList{"--version --json"});
    }

    // A terminal fix starts only the launcher, and only with one of
    // Model.js's TERMINAL_SCRIPTS: the launcher runs its argument as shell.
    void terminalFixStartsTheLauncherOnly() {
        QTemporaryDir dir;
        auto o = options(dir, dir.filePath("index.json"));
        QVERIFY(write(o.launcher, "#!/bin/sh\nprintf '%s' \"$1\" > '" + dir.filePath("launched").toUtf8() + "'\n"));
        QVERIFY(QFile::setPermissions(o.launcher, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner));

        const QJSValue scripts = modelJs("TERMINAL_SCRIPTS");
        QCOMPARE(Backend::terminalScripts().size(), scripts.property("length").toInt());
        QVERIFY(Backend::terminalScripts().size() >= 2);
        const QString init = modelJs("INIT_SCRIPT").toString();
        QVERIFY(Backend::terminalScripts().contains(init));

        Backend backend(o);
        QVERIFY(!backend.openTerminal({"sh", "-c", "true"}));
        QVERIFY(!backend.openTerminal({"bash", "true"}));
        QVERIFY(!backend.openTerminal({Backend::TerminalLauncher, "  "}));
        QVERIFY(!backend.openTerminal({Backend::TerminalLauncher, "seldon init --defaults"}));
        QVERIFY(!backend.openTerminal({Backend::TerminalLauncher, init + "; rm -rf ~"}));
        QVERIFY(!backend.openTerminal({Backend::TerminalLauncher, init, "x"}));
        QTest::qWait(200);
        QVERIFY(!QFile::exists(dir.filePath("launched")));
        QVERIFY(backend.openTerminal({Backend::TerminalLauncher, init}));
        QTRY_COMPARE(QString::fromUtf8(read(dir.filePath("launched"))), init);
    }

    // ---- The window -----------------------------------------------------

    // The fixture in dev mode with the default accent: the Changes view with
    // rows, no QML warning; the screenshot goes to SELDON_DESK_SCREENSHOT
    // when set (docs/images/).
    void windowShowsTheFixture() {
        QTemporaryDir dir;
        auto window = load(dir, options(dir, Fixture, true), "en");
        QVERIFY(window->window);
        QQuickItem *list = find(window.get(), "changesList");
        QVERIFY(list);
        QTRY_VERIFY(list->property("count").toInt() > 0);
        QVERIFY(find(window.get(), "changesView")->isVisible());
        QVERIFY(!find(window.get(), "stateCard")->isVisible());
        QVERIFY(find(window.get(), "devModeLine")->isVisible());
        QTest::qWait(300);
        const QImage image = window->window->grabWindow();
        QVERIFY(!image.isNull());
        const QString target = qEnvironmentVariable("SELDON_DESK_SCREENSHOT");
        if (!target.isEmpty())
            QVERIFY(image.save(target));
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // Every state shows its title and one fix, in German too.
    void windowShowsEachState_data() {
        QTest::addColumn<QString>("language");
        QTest::addColumn<QString>("state");
        QTest::addColumn<QString>("title");
        QTest::addColumn<QString>("fix");
        QTest::newRow("engine missing") << "en" << "engineMissing" << "The Seldon engine is missing" << "Install the engine";
        QTest::newRow("not initialised") << "en" << "notInitialised" << "Create your logbook" << "Create logbook";
        QTest::newRow("index missing") << "en" << "indexMissing" << "No index yet" << "Build index";
        QTest::newRow("index stale") << "en" << "indexStale" << "" << "Capture now";
        QTest::newRow("engine missing, de") << "de" << "engineMissing" << "Die Seldon-Engine fehlt" << "Engine installieren";
        QTest::newRow("index missing, de") << "de" << "indexMissing" << "Noch kein Index" << "Index erstellen";
    }

    void windowShowsEachState() {
        QFETCH(QString, language);
        QFETCH(QString, state);
        QFETCH(QString, title);
        QFETCH(QString, fix);
        QTemporaryDir dir;
        auto o = options(dir, dir.filePath("index.json"));
        if (state == "engineMissing")
            o.engine = dir.filePath("no-such-seldon");
        if (state == "notInitialised")
            QVERIFY(write(dir.filePath("status-exit"), "3\n"));
        if (state == "indexStale")
            QVERIFY(write(dir.filePath("index.json"), read(Fixture)));
        auto window = load(dir, o, language);
        QVERIFY(window->window);
        QQuickItem *card = find(window.get(), "stateCard");
        QTRY_COMPARE(window->backend->status(), state);
        QTRY_VERIFY(card->isVisible());
        if (!title.isEmpty())
            QTRY_COMPARE(find(window.get(), "stateTitle")->property("text").toString(), title);
        QTRY_COMPARE(find(window.get(), "stateFix")->property("text").toString(), fix);
        if (state == "indexStale")
            QVERIFY(find(window.get(), "changesView")->isVisible());
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // Enter opens the selected change's form at Link (the engine proposes a
    // case); Enter arms, Enter again runs exactly the contract's form.
    void resolvingArmsTwice() {
        QTemporaryDir dir;
        QVERIFY(write(dir.filePath("index.json"), read(Fixture)));
        auto window = load(dir, options(dir, dir.filePath("index.json")), "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QTRY_COMPARE(window->backend->engineState(), QString("present"));
        QQuickItem *view = find(window.get(), "changesView");
        view->setProperty("selectedId", "01M3VTGNY0NZG4AY80814WSKGR");
        QQuickItem *button = find(window.get(), "resolveButton");
        QTRY_VERIFY(button->isVisible());
        QCOMPARE(button->property("text").toString(), QString("Link"));

        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_VERIFY(find(window.get(), "caseBox")->hasActiveFocus());
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_COMPARE(button->property("text").toString(), QString("Press again to confirm"));
        QVERIFY(!calls(dir).contains("drift link 01M3VTGNY0NZG4AY80814WSKGR C-2026-005 --json"));
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_VERIFY(calls(dir).contains("drift link 01M3VTGNY0NZG4AY80814WSKGR C-2026-005 --json"));
        QTRY_COMPARE(find(window.get(), "formAnswer")->property("text").toString(), QString("Linked 1 event to C-2026-005"));
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // Esc disarms; another selection does too.
    void anotherKeyDisarms() {
        QTemporaryDir dir;
        QVERIFY(write(dir.filePath("index.json"), read(Fixture)));
        auto window = load(dir, options(dir, dir.filePath("index.json")), "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QQuickItem *view = find(window.get(), "changesView");
        view->setProperty("selectedId", "01M3VNJ9JGZ9169T01XCW16FT0");
        QQuickItem *button = find(window.get(), "resolveButton");
        QTRY_VERIFY(button->isVisible());
        // A crisis without a proposal starts at Explain: type, Enter arms.
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_VERIFY(find(window.get(), "reasonField")->hasActiveFocus());
        for (const char c : QByteArray("testing ollama"))
            QTest::keyClick(window->window, c);
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_COMPARE(button->property("text").toString(), QString("Press again to confirm"));
        QTest::keyClick(window->window, Qt::Key_Escape);
        QTest::keyClick(window->window, Qt::Key_Escape);
        QTRY_COMPARE(button->property("text").toString(), QString("Explain"));
        QVERIFY(!calls(dir).join('\n').contains("drift explain"));
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // ? shows the keys; Q with a typed text asks first, Esc stays.
    void helpAndQuitAsk() {
        QTemporaryDir dir;
        QVERIFY(write(dir.filePath("index.json"), read(Fixture)));
        auto window = load(dir, options(dir, dir.filePath("index.json")), "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QQuickItem *help = find(window.get(), "helpOverlay");
        QVERIFY(!help->isVisible());
        QTest::keyClick(window->window, '?', Qt::ShiftModifier);
        QTRY_VERIFY(help->isVisible());
        QTest::keyClick(window->window, Qt::Key_Escape);
        QTRY_VERIFY(!help->isVisible());

        find(window.get(), "changesView")->setProperty("selectedId", "01M3VNJ9JGZ9169T01XCW16FT0");
        QTRY_VERIFY(find(window.get(), "resolveButton")->isVisible());
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_VERIFY(find(window.get(), "reasonField")->hasActiveFocus());
        // Q in the field is a letter of the text.
        QTest::keyClick(window->window, 'q');
        QCOMPARE(find(window.get(), "reasonField")->property("text").toString(), QString("q"));
        QTest::keyClick(window->window, Qt::Key_Escape);
        QTRY_VERIFY(!find(window.get(), "reasonField")->hasActiveFocus());
        QTest::keyClick(window->window, 'q');
        QTRY_VERIFY(find(window.get(), "quitAsk")->isVisible());
        QTest::keyClick(window->window, Qt::Key_Escape);
        QTRY_VERIFY(!find(window.get(), "quitAsk")->isVisible());
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // ---- The window: review round 1 (the reviewer's reproductions) ------

    // The pick is a case id: a new index that drops a case above it keeps
    // the form armed for the same case, and the second press runs exactly
    // the armed argv.
    void reviewCaseShiftsWhileArmed() {
        QTemporaryDir dir;
        const QString path = dir.filePath("index.json");
        QVERIFY(write(path, read(Fixture)));
        auto window = load(dir, options(dir, path), "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QTRY_COMPARE(window->backend->engineState(), QString("present"));
        find(window.get(), "changesView")->setProperty("selectedId", "01M3VTGNY0NZG4AY80814WSKGR");
        QQuickItem *detail = find(window.get(), "changeDetail");
        QTRY_VERIFY(find(window.get(), "resolveButton")->isVisible());
        detail->setProperty("caseId", "C-2026-004");
        QCOMPARE(detail->property("caseAt").toInt(), 2);
        const QString before = detail->property("summary").toString();
        QMetaObject::invokeMethod(detail, "press");
        QTRY_VERIFY(detail->property("armed").toBool());

        QVERIFY(writeFixture(path + ".tmp", dropFirstCase));
        QVERIFY(QFile::remove(path));
        QVERIFY(QFile::rename(path + ".tmp", path));
        QTRY_COMPARE(window->backend->index()["cases"].toObject()["active"].toArray().size(), 1);
        QTRY_COMPARE(detail->property("caseAt").toInt(), 1);
        QVERIFY(detail->property("armed").toBool());
        QCOMPARE(detail->property("summary").toString(), before);
        QMetaObject::invokeMethod(detail, "press");
        QTRY_VERIFY(calls(dir).contains("drift link 01M3VTGNY0NZG4AY80814WSKGR C-2026-004 --json"));
        QCOMPARE(calls(dir).filter("drift link").size(), 1);
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // The armed case leaves the index before the second press: the form
    // disarms and the second press runs nothing.
    void armedCaseLeavingDisarms() {
        QTemporaryDir dir;
        const QString path = dir.filePath("index.json");
        QVERIFY(write(path, read(Fixture)));
        auto window = load(dir, options(dir, path), "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QTRY_COMPARE(window->backend->engineState(), QString("present"));
        find(window.get(), "changesView")->setProperty("selectedId", "01M3VTGNY0NZG4AY80814WSKGR");
        QQuickItem *detail = find(window.get(), "changeDetail");
        QTRY_VERIFY(find(window.get(), "resolveButton")->isVisible());
        detail->setProperty("caseId", "C-2026-003");
        QMetaObject::invokeMethod(detail, "press");
        QTRY_VERIFY(detail->property("armed").toBool());

        QVERIFY(writeFixture(path + ".tmp", dropFirstCase));
        QVERIFY(QFile::remove(path));
        QVERIFY(QFile::rename(path + ".tmp", path));
        QTRY_VERIFY(!detail->property("armed").toBool());
        QMetaObject::invokeMethod(detail, "press");
        QTest::qWait(500);
        QVERIFY(!calls(dir).join('\n').contains("drift link"));
        QVERIFY(!detail->property("armed").toBool());
        QVERIFY(!detail->property("error").toString().isEmpty());
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // The action changes while a resolution runs: the answer still ends
    // `pending` (it is matched to the argv that ran), and another selection
    // clears it as well.
    void reviewPendingSticks() {
        QTemporaryDir dir;
        const QString path = dir.filePath("index.json");
        QVERIFY(write(path, read(Fixture)));
        auto o = options(dir, path);
        QByteArray script = read(o.engine);
        script.replace("drift) echo", "drift) sleep 1; echo");
        QVERIFY(write(o.engine, script));
        auto window = load(dir, o, "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QTRY_COMPARE(window->backend->engineState(), QString("present"));
        QQuickItem *view = find(window.get(), "changesView");
        view->setProperty("selectedId", "01M3VTGNY0NZG4AY80814WSKGR");
        QQuickItem *detail = find(window.get(), "changeDetail");
        QQuickItem *button = find(window.get(), "resolveButton");
        QTRY_VERIFY(button->isVisible());
        QMetaObject::invokeMethod(detail, "press");
        QMetaObject::invokeMethod(detail, "press");
        QTRY_VERIFY(detail->property("pending").toBool());
        // The form is locked while the engine works.
        QVERIFY(!button->isEnabled());
        QVERIFY(!find(window.get(), "action-explain")->isEnabled());
        QVERIFY(!find(window.get(), "caseBox")->isEnabled());
        detail->setProperty("action", "explain");
        QTRY_VERIFY_WITH_TIMEOUT(!detail->property("pending").toBool(), 5000);
        QCOMPARE(find(window.get(), "formAnswer")->property("text").toString(), QString("Linked 1 event to C-2026-005"));
        QVERIFY(button->isEnabled());

        // A second run, and another change selected before it answers.
        detail->setProperty("action", "link");
        QMetaObject::invokeMethod(detail, "press");
        QMetaObject::invokeMethod(detail, "press");
        QTRY_VERIFY(detail->property("pending").toBool());
        view->setProperty("selectedId", "01M3VNJ9JGZ9169T01XCW16FT0");
        QTRY_VERIFY(!detail->property("pending").toBool());
        QTRY_VERIFY(button->isEnabled());
        QTRY_VERIFY(!window->backend->busy());
        QCOMPARE(find(window.get(), "formAnswer")->property("text").toString(), QString());
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // Markup in a case title (the Link picker) and in an engine error shows
    // as the characters it is; every text item of the window is plain.
    void reviewRichText() {
        QTemporaryDir dir;
        const QString path = dir.filePath("index.json");
        const QString markup = "<font color=\"#ff00ff\" size=\"7\">RICH</font><img src=\"x.png\">";
        QVERIFY(writeFixture(path, [&](QJsonObject &index) {
            QJsonObject cases = index["cases"].toObject();
            QJsonArray active = cases["active"].toArray();
            QJsonObject first = active[0].toObject();
            first["title"] = markup;
            active[0] = first;
            cases["active"] = active;
            index["cases"] = cases;
        }));
        auto o = options(dir, path);
        QByteArray script = read(o.engine);
        script.replace("capture) echo '{\"written\":0,\"collectors\":[]}' ;;",
                       "capture) echo '<font color=\"#00ff00\" size=\"7\">ERR</font>' >&2; exit 2 ;;");
        QVERIFY(write(o.engine, script));
        auto window = load(dir, o, "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QTRY_COMPARE(window->backend->engineState(), QString("present"));
        find(window.get(), "changesView")->setProperty("selectedId", "01M3VTGNY0NZG4AY80814WSKGR");
        QTRY_VERIFY(find(window.get(), "resolveButton")->isVisible());
        QTest::keyClick(window->window, Qt::Key_Space);
        QTRY_VERIFY_WITH_TIMEOUT(find(window.get(), "captureAnswer")->property("text").toString().contains("ERR"), 3000);
        QQuickItem *caseBox = find(window.get(), "caseBox");
        caseBox->setProperty("currentIndex", 1);
        QObject *popup = caseBox->property("popup").value<QObject *>();
        popup->setProperty("visible", true);
        QTRY_VERIFY(find(window.get(), "caseOption"));
        QTest::qWait(300);

        // Every text but the window's own fixed words (the catalog's keys,
        // e.g. a placeholder or the check box Material draws) is plain.
        const QJsonObject fixed = QJsonDocument::fromJson(read(":/i18n/de.json")).object();
        QStringList rich, literal;
        for (QQuickItem *item : allItems(window.get())) {
            if (!item->inherits("QQuickText"))
                continue;
            const QString text = item->property("text").toString();
            if (text.isEmpty() || fixed.contains(text))
                continue;
            if (item->property("textFormat").toInt() != Qt::PlainText)
                rich << QString::fromUtf8(item->metaObject()->className()) + ": " + text;
            else if (text.contains("<font"))
                literal << text;
        }
        QVERIFY2(rich.isEmpty(), qPrintable(rich.join('\n')));
        // The capture error, the picker's own text and its popup's item.
        QVERIFY2(literal.size() >= 3, qPrintable(literal.join('\n')));
        QVERIFY(literal.filter("ERR").size() >= 1);
        QVERIFY(literal.filter("RICH").size() >= 2);
        popup->setProperty("visible", false);
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // The stale strip sits above the list, its fix inside it.
    void reviewStaleLayout() {
        QTemporaryDir dir;
        const QString path = dir.filePath("index.json");
        QVERIFY(write(path, read(Fixture)));
        auto window = load(dir, options(dir, path), "en");
        QVERIFY(window->window);
        QTRY_COMPARE(window->backend->status(), QString("indexStale"));
        QTest::qWait(300);
        QQuickItem *fix = find(window.get(), "stateFix");
        QQuickItem *card = find(window.get(), "stateCard");
        QQuickItem *view = find(window.get(), "changesView");
        QVERIFY(fix->isVisible() && card->isVisible() && view->isVisible());
        const QRectF f = fix->mapRectToScene(QRectF(0, 0, fix->width(), fix->height()));
        const QRectF c = card->mapRectToScene(QRectF(0, 0, card->width(), card->height()));
        const QRectF v = view->mapRectToScene(QRectF(0, 0, view->width(), view->height()));
        const QString where = QString("card %1+%2, fix %3+%4, view %5").arg(c.y()).arg(c.height()).arg(f.y()).arg(f.height()).arg(v.y());
        QVERIFY2(c.contains(f), qPrintable(where));
        QVERIFY2(f.bottom() <= v.top(), qPrintable(where));
        QVERIFY2(c.bottom() <= v.top(), qPrintable(where));
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // Any change to an armed form disarms it: the text, the action, the
    // case, "only this event". Space in the case picker is the picker's.
    void formChangesDisarm() {
        QTemporaryDir dir;
        QVERIFY(write(dir.filePath("index.json"), read(Fixture)));
        auto window = load(dir, options(dir, dir.filePath("index.json")), "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QTRY_COMPARE(window->backend->engineState(), QString("present"));
        QQuickItem *view = find(window.get(), "changesView");
        QQuickItem *detail = find(window.get(), "changeDetail");
        view->setProperty("selectedId", "01M3VNJ9JGZ9169T01XCW16FT0");
        QTRY_VERIFY(find(window.get(), "resolveButton")->isVisible());
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_VERIFY(find(window.get(), "reasonField")->hasActiveFocus());
        for (const char c : QByteArray("abc"))
            QTest::keyClick(window->window, c);
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_VERIFY(detail->property("armed").toBool());
        QTest::keyClick(window->window, 'd');
        QTRY_VERIFY(!detail->property("armed").toBool());
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_VERIFY(detail->property("armed").toBool());
        QQuickItem *dismiss = find(window.get(), "action-dismiss");
        QTest::mouseClick(window->window, Qt::LeftButton, {}, dismiss->mapToScene(QPointF(dismiss->width() / 2, dismiss->height() / 2)).toPoint());
        QTRY_COMPARE(detail->property("action").toString(), QString("dismiss"));
        QVERIFY(!detail->property("armed").toBool());
        QMetaObject::invokeMethod(detail, "press");
        QTRY_VERIFY(detail->property("armed").toBool());
        detail->setProperty("only", true);
        QVERIFY(!detail->property("armed").toBool());

        view->setProperty("selectedId", "01M3VTGNY0NZG4AY80814WSKGR");
        QTRY_COMPARE(detail->property("action").toString(), QString("link"));
        QMetaObject::invokeMethod(detail, "press");
        QTRY_VERIFY(detail->property("armed").toBool());
        detail->setProperty("caseId", "C-2026-004");
        QVERIFY(!detail->property("armed").toBool());

        QQuickItem *caseBox = find(window.get(), "caseBox");
        caseBox->forceActiveFocus();
        QTRY_VERIFY(caseBox->hasActiveFocus());
        QTest::keyClick(window->window, Qt::Key_Space);
        QTest::qWait(300);
        QVERIFY(!calls(dir).join('\n').contains("capture"));
        QVERIFY(!calls(dir).join('\n').contains("drift"));
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // Another change while a typed reason is unsent asks first; Esc keeps
    // the text, a second try drops it.
    void anotherChangeAsksBeforeDroppingText() {
        QTemporaryDir dir;
        QVERIFY(write(dir.filePath("index.json"), read(Fixture)));
        auto window = load(dir, options(dir, dir.filePath("index.json")), "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QQuickItem *view = find(window.get(), "changesView");
        QQuickItem *detail = find(window.get(), "changeDetail");
        QQuickItem *ask = find(window.get(), "discardAsk");
        const QString id = "01M3VNJ9JGZ9169T01XCW16FT0";
        view->setProperty("selectedId", id);
        QTRY_VERIFY(find(window.get(), "resolveButton")->isVisible());
        QTest::keyClick(window->window, Qt::Key_Return);
        QTRY_VERIFY(find(window.get(), "reasonField")->hasActiveFocus());
        for (const char c : QByteArray("mine"))
            QTest::keyClick(window->window, c);
        QTest::keyClick(window->window, Qt::Key_Escape);
        QTRY_VERIFY(!find(window.get(), "reasonField")->hasActiveFocus());

        QTest::keyClick(window->window, 'j');
        QTRY_VERIFY(ask->isVisible());
        QCOMPARE(view->property("selectedId").toString(), id);
        QCOMPARE(detail->property("reason").toString(), QString("mine"));
        QTest::keyClick(window->window, Qt::Key_Escape);
        QTRY_VERIFY(!ask->isVisible());
        QCOMPARE(detail->property("reason").toString(), QString("mine"));

        QTest::keyClick(window->window, 'j');
        QTRY_VERIFY(ask->isVisible());
        QTest::keyClick(window->window, 'j');
        QTRY_VERIFY(view->property("selectedId").toString() != id);
        QVERIFY(!ask->isVisible());
        QCOMPARE(detail->property("reason").toString(), QString());
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    // ADR-0051: an index of a newer contract that this window can read
    // shows, with the quiet note to update.
    void newerContractShowsTheNotice() {
        QTemporaryDir dir;
        const QString path = dir.filePath("index.json");
        QVERIFY(writeFixture(path, [](QJsonObject &index) {
            index["contractVersion"] = 3;
            index["contractReadableFrom"] = 2;
        }));
        auto window = load(dir, options(dir, path, true), "en");
        QVERIFY(window->window);
        QTRY_VERIFY(find(window.get(), "changesList")->property("count").toInt() > 0);
        QQuickItem *notice = find(window.get(), "contractNotice");
        QTRY_VERIFY(notice->isVisible());
        QCOMPARE(notice->property("text").toString(),
                 QString("The engine writes index v3; this window reads v2 — update Seldon."));

        QVERIFY(write(path + ".tmp", read(Fixture)));
        QVERIFY(QFile::remove(path));
        QVERIFY(QFile::rename(path + ".tmp", path));
        QTRY_VERIFY(!notice->isVisible());
        QVERIFY2(messages.isEmpty(), qPrintable(messages.join('\n')));
    }

    void init() { messages.clear(); }

private:
    struct Window {
        std::unique_ptr<Translator> translator;
        std::unique_ptr<Theme> theme;
        std::unique_ptr<Backend> backend;
        std::unique_ptr<QQmlApplicationEngine> engine;
        QQuickWindow *window = nullptr;
        ~Window() {
            engine.reset();
            if (translator)
                QCoreApplication::removeTranslator(translator.get());
        }
    };

    std::unique_ptr<Window> load(const QTemporaryDir &dir, const Backend::Options &o, const QString &language) {
        auto w = std::make_unique<Window>();
        write(dir.filePath("language"), language.toUtf8() + "\n");
        w->translator = std::make_unique<Translator>(dir.filePath("language"));
        QCoreApplication::installTranslator(w->translator.get());
        // An empty theme directory: the default accent and the shell's
        // default palette, whatever the machine runs.
        QDir(dir.path()).mkpath("omarchy");
        w->theme = std::make_unique<Theme>(dir.filePath("omarchy"));
        w->backend = std::make_unique<Backend>(o);
        w->engine = std::make_unique<QQmlApplicationEngine>();
        w->engine->rootContext()->setContextProperty("translator", w->translator.get());
        w->engine->rootContext()->setContextProperty("theme", w->theme.get());
        w->engine->rootContext()->setContextProperty("backend", w->backend.get());
        w->engine->load(QUrl("qrc:/Main.qml"));
        if (w->engine->rootObjects().isEmpty())
            return w;
        w->window = qobject_cast<QQuickWindow *>(w->engine->rootObjects().first());
        if (w->window) {
            w->window->requestActivate();
            if (!QTest::qWaitForWindowExposed(w->window) || !QTest::qWaitForWindowActive(w->window))
                w->window = nullptr;
        }
        return w;
    }

    // By objectName: through the object tree, then through the visual one
    // (a Repeater's delegates are only in the latter).
    static QQuickItem *find(Window *w, const char *name) {
        if (!w || !w->window)
            return nullptr;
        if (QQuickItem *item = w->window->findChild<QQuickItem *>(name))
            return item;
        return findItem(w->window->contentItem(), QString::fromUtf8(name));
    }

    // Every item of the window, the open popups' included.
    static QList<QQuickItem *> allItems(Window *w) {
        QList<QQuickItem *> out = w->window->findChildren<QQuickItem *>();
        QList<QQuickItem *> todo{w->window->contentItem()};
        while (!todo.isEmpty()) {
            QQuickItem *item = todo.takeLast();
            if (!out.contains(item))
                out << item;
            todo << item->childItems();
        }
        return out;
    }

    static QQuickItem *findItem(QQuickItem *item, const QString &name) {
        if (!item)
            return nullptr;
        if (item->objectName() == name)
            return item;
        for (QQuickItem *child : item->childItems())
            if (QQuickItem *found = findItem(child, name))
                return found;
        return nullptr;
    }
};

int main(int argc, char *argv[]) {
    qInstallMessageHandler(collect);
    QGuiApplication app(argc, argv);
    QQuickStyle::setStyle("Material");
    SeldonDeskTests tests;
    return QTest::qExec(&tests, argc, argv);
}

#include "seldon-desk_tests.moc"
