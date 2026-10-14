#pragma once

#include <QFileSystemWatcher>
#include <QFutureWatcher>
#include <QJsonObject>
#include <QObject>
#include <QProcess>
#include <QStringList>
#include <QTimer>

// The window's side of the contract (docs/CONTRACT.md): it reads index.json,
// which the engine alone writes, and runs `seldon … --json` with argument
// lists checked against the forms below. It never parses the logbook.
//
// status is one of Model.js's STATUSES ("ok", "engineMissing",
// "notInitialised", "indexMissing", "indexStale", "contractMismatch") or
// "loading" before the first read has finished.
class Backend : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString status READ status NOTIFY stateChanged)
    Q_PROPERTY(QString engineState READ engineState NOTIFY stateChanged)
    Q_PROPERTY(QString engineVersion READ engineVersion NOTIFY stateChanged)
    Q_PROPERTY(QString fileState READ fileState NOTIFY stateChanged)
    Q_PROPERTY(QString parseError READ parseError NOTIFY stateChanged)
    Q_PROPERTY(qint64 indexSize READ indexSize NOTIFY stateChanged)
    Q_PROPERTY(int indexContractVersion READ indexContractVersion NOTIFY stateChanged)
    Q_PROPERTY(bool contractNewer READ contractNewer NOTIFY stateChanged)
    Q_PROPERTY(bool hasIndex READ hasIndex NOTIFY indexChanged)
    Q_PROPERTY(QJsonObject index READ index NOTIFY indexChanged)
    Q_PROPERTY(QString indexPath READ indexPath CONSTANT)
    Q_PROPERTY(bool devMode READ devMode CONSTANT)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(double nowMs READ nowMs NOTIFY stateChanged)

public:
    // CONTRACT.md: the contract this app reads, the index's size budget, and
    // SPEC-PLUGIN §3's age after which an index is stale. Model.js says the
    // same (CONTRACT_VERSION, INDEX_MAX_CHARS, STALE_AFTER_MS); a test keeps
    // the two equal.
    static constexpr int ContractVersion = 2;
    static constexpr qint64 IndexMaxBytes = 1000000;
    static constexpr qint64 StaleAfterMs = 2LL * 60 * 60 * 1000;
    // The launcher a terminal fix runs in (Model.terminalArgv's first word).
    static constexpr const char *TerminalLauncher = "omarchy-launch-floating-terminal-with-presentation";

    struct Options {
        // The index to read; defaultIndexPath() in the app, a temp file in tests.
        QString indexPath;
        // The engine program; "seldon" from PATH in the app, a fake in tests.
        QString engine = "seldon";
        // The program a terminal fix starts; TerminalLauncher in the app.
        QString launcher = TerminalLauncher;
        // CONTRACT.md rule 1, SELDON_INDEX: render the file, probe the engine's
        // version, run nothing else. `now` is SELDON_NOW.
        bool devMode = false;
        QString now;
    };

    struct Parsed {
        // "", "missing", "unreadable", "empty", "size", "parse", "shape",
        // "contract" (Model.parseIndex's errors, plus the two of the file).
        QString error;
        QString detail;
        qint64 size = 0;
        int contractVersion = 0;
        int readableFrom = 0;
        bool newer = false;
        QJsonObject index;
    };

    explicit Backend(const Options &options, QObject *parent = nullptr);
    ~Backend() override;

    // ${XDG_STATE_HOME:-$HOME/.local/state}/seldon/index.json (CONTRACT.md rule 1).
    static QString defaultIndexPath();
    // The app's options: the state index, or SELDON_INDEX and SELDON_NOW.
    static Options fromEnvironment();

    // Pure and thread-safe: the worker thread reads and parses with these.
    static Parsed readIndex(const QString &path);
    static Parsed parseIndex(const QByteArray &bytes);
    // "" when `args` (without the program) is a form this app may run, else
    // why not.
    static QString validateArgs(const QStringList &args);
    // Model.deriveStatus's precedence, with "loading" while the file is.
    static QString deriveStatus(const QString &engineState, const QString &fileState, const Parsed &parsed,
                                bool engineNotInitialised, qint64 nowMs);
    static bool isStale(const QString &generatedAt, qint64 nowMs);

    QString status() const { return m_status; }
    QString engineState() const { return m_engineState; }
    QString engineVersion() const { return m_engineVersion; }
    QString fileState() const { return m_fileState; }
    QString parseError() const { return m_parsed.error; }
    qint64 indexSize() const { return m_parsed.size; }
    int indexContractVersion() const { return m_parsed.contractVersion; }
    bool contractNewer() const { return m_parsed.newer; }
    bool hasIndex() const { return m_parsed.error.isEmpty() && !m_parsed.index.isEmpty(); }
    QJsonObject index() const { return hasIndex() ? m_parsed.index : QJsonObject(); }
    QString indexPath() const { return m_options.indexPath; }
    bool devMode() const { return m_options.devMode; }
    bool busy() const { return m_process.state() != QProcess::NotRunning || !m_queue.isEmpty(); }
    double nowMs() const { return double(currentMs()); }

    // Queue one engine call; false when validateArgs refuses it, in dev mode,
    // or without an engine.
    Q_INVOKABLE bool run(const QStringList &args);
    // The one fix of a state that the engine does itself: indexMissing builds
    // the index (`status`), indexStale captures. Terminal fixes go through
    // openTerminal.
    Q_INVOKABLE bool fix();
    // A terminal fix: exactly [TerminalLauncher, script], as
    // Model.terminalArgv returns it for one of Model.js's fixed scripts.
    Q_INVOKABLE bool openTerminal(const QStringList &argv);
    // Probe the engine and read the index again.
    Q_INVOKABLE void recheck();

signals:
    void stateChanged();
    void indexChanged();
    void busyChanged();
    // Every engine call's answer, for the view's result line
    // (Model.driftResult and friends read it).
    void finished(const QStringList &args, int exitCode, const QString &output, const QString &errorOutput);

private:
    void probe();
    void reload();
    void rearm();
    void ingest(const Parsed &parsed);
    void startNext();
    void callFinished(int exitCode, QProcess::ExitStatus exitStatus);
    void refreshStatus();
    qint64 currentMs() const;

    Options m_options;
    QString m_status = "loading";
    QString m_engineState = "unknown";
    QString m_engineVersion;
    QString m_fileState = "loading";
    Parsed m_parsed;
    bool m_engineNotInitialised = false;
    qint64 m_notInitialisedAtMs = 0;
    bool m_statusAsked = false;

    QFileSystemWatcher m_watcher;
    QTimer m_debounce;
    QTimer m_clock;
    QFutureWatcher<Parsed> m_reading;
    bool m_readAgain = false;

    QProcess m_probe;
    QProcess m_process;
    QTimer m_timeout;
    QTimer m_repoll;
    int m_repollLeft = 0;
    QList<QStringList> m_queue;
    QStringList m_running;
};
