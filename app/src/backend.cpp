#include "backend.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonValue>
#include <QRegularExpression>
#include <QtConcurrent/QtConcurrentRun>
#include <cmath>

namespace {

// CONTRACT.md's ids and words, as schema/event.schema.json and
// schema/case.schema.json write them (Model.js EVENT_ID, CASE_ID, ZONES,
// RISKS, AREA).
const QRegularExpression EventId(QStringLiteral("^[0-7][0-9A-HJKMNP-TV-Z]{25}$"));
const QRegularExpression CaseId(QStringLiteral("^C-[0-9]{4}-[0-9]{3,}$"));
const QRegularExpression Area(QStringLiteral("^[a-z0-9][a-z0-9-]*$"));
const QStringList Zones{"green", "yellow", "red"};
const QStringList Risks{"R0", "R1", "R2", "R3"};

// An engine call that has not answered in two minutes is stuck; capture is
// the slowest call and takes seconds.
constexpr int CallTimeoutMs = 120000;
// After a terminal fix the engine may appear at any moment (its install
// runs in that terminal), so look for it every few seconds for a while.
constexpr int RepollMs = 5000;
constexpr int RepollTries = 120;

qint64 timeMs(const QString &text) {
    if (text.isEmpty())
        return -1;
    const QDateTime at = QDateTime::fromString(text, Qt::ISODateWithMs);
    return at.isValid() ? at.toMSecsSinceEpoch() : -1;
}

bool isInteger(const QJsonValue &value) {
    return value.isDouble() && std::floor(value.toDouble()) == value.toDouble() && std::isfinite(value.toDouble());
}

} // namespace

Backend::Backend(const Options &options, QObject *parent) : QObject(parent), m_options(options) {
    m_debounce.setSingleShot(true);
    m_debounce.setInterval(80);
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, [this] { m_debounce.start(); });
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, this, [this] { m_debounce.start(); });
    connect(&m_debounce, &QTimer::timeout, this, &Backend::reload);
    connect(&m_reading, &QFutureWatcher<Parsed>::finished, this, [this] {
        ingest(m_reading.result());
        if (m_readAgain) {
            m_readAgain = false;
            reload();
        }
    });

    // An index turns stale with time alone, so the status is looked at again
    // every half minute.
    m_clock.setInterval(30000);
    connect(&m_clock, &QTimer::timeout, this, &Backend::refreshStatus);
    m_clock.start();

    connect(&m_probe, &QProcess::finished, this, [this](int exitCode, QProcess::ExitStatus exitStatus) {
        const QJsonObject answer = QJsonDocument::fromJson(m_probe.readAllStandardOutput()).object();
        // A seldon that cannot even report its version is no usable engine.
        const bool present = exitStatus == QProcess::NormalExit && exitCode == 0;
        m_engineState = present ? "present" : "missing";
        m_engineVersion = present ? answer.value("version").toString() : QString();
        refreshStatus();
        // Without an index, ask the engine once whether a logbook exists:
        // `status` answers 3 when not, and writes the index when it does.
        if (present && !m_statusAsked && (m_fileState == "missing" || m_fileState == "invalid")) {
            m_statusAsked = true;
            run({"status", "--json"});
        }
    });
    connect(&m_probe, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error != QProcess::FailedToStart)
            return;
        m_engineState = "missing";
        m_engineVersion.clear();
        refreshStatus();
    });

    m_timeout.setSingleShot(true);
    m_timeout.setInterval(CallTimeoutMs);
    connect(&m_timeout, &QTimer::timeout, &m_process, &QProcess::kill);
    connect(&m_process, &QProcess::finished, this, &Backend::callFinished);
    connect(&m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error != QProcess::FailedToStart)
            return;
        m_timeout.stop();
        const QStringList args = m_running;
        m_running.clear();
        m_engineState = "missing";
        refreshStatus();
        emit finished(args, -1, {}, m_process.errorString());
        startNext();
    });

    m_repoll.setInterval(RepollMs);
    connect(&m_repoll, &QTimer::timeout, this, [this] {
        if (m_engineState != "missing" || --m_repollLeft <= 0)
            m_repoll.stop();
        else
            probe();
    });

    reload();
    probe();
}

Backend::~Backend() {
    for (QProcess *process : {&m_probe, &m_process}) {
        if (process->state() != QProcess::NotRunning) {
            process->kill();
            process->waitForFinished(1000);
        }
    }
    m_reading.waitForFinished();
}

QString Backend::defaultIndexPath() {
    const QString state = qEnvironmentVariable("XDG_STATE_HOME");
    return (state.isEmpty() ? QDir::homePath() + "/.local/state" : state) + "/seldon/index.json";
}

Backend::Options Backend::fromEnvironment() {
    Options options;
    const QString devIndex = qEnvironmentVariable("SELDON_INDEX");
    options.devMode = !devIndex.isEmpty();
    options.indexPath = options.devMode ? QFileInfo(devIndex).absoluteFilePath() : defaultIndexPath();
    options.now = options.devMode ? qEnvironmentVariable("SELDON_NOW") : QString();
    return options;
}

// ---- Reading the index ------------------------------------------------------

Backend::Parsed Backend::readIndex(const QString &path) {
    Parsed parsed;
    QFile file(path);
    if (!file.exists()) {
        parsed.error = "missing";
        return parsed;
    }
    if (!file.open(QIODevice::ReadOnly)) {
        parsed.error = "unreadable";
        parsed.detail = file.errorString();
        return parsed;
    }
    // CONTRACT.md rule 5: the index stays under 1 MB. A longer file is
    // refused before it is read whole, let alone parsed.
    parsed.size = file.size();
    if (parsed.size >= IndexMaxBytes) {
        parsed.error = "size";
        return parsed;
    }
    const QByteArray bytes = file.read(IndexMaxBytes);
    if (bytes.size() >= IndexMaxBytes) {
        parsed.size = bytes.size();
        parsed.error = "size";
        return parsed;
    }
    return parseIndex(bytes);
}

// Model.parseIndex in C++: the same errors in the same order, and only a
// contract this app reads yields an index (CONTRACT.md rule 3, ADR-0051).
Backend::Parsed Backend::parseIndex(const QByteArray &bytes) {
    Parsed parsed;
    parsed.size = bytes.size();
    if (bytes.size() >= IndexMaxBytes) {
        parsed.error = "size";
        return parsed;
    }
    if (bytes.trimmed().isEmpty()) {
        parsed.error = "empty";
        return parsed;
    }
    QJsonParseError error;
    const QJsonDocument document = QJsonDocument::fromJson(bytes, &error);
    if (error.error != QJsonParseError::NoError) {
        parsed.error = "parse";
        parsed.detail = error.errorString();
        return parsed;
    }
    if (!document.isObject()) {
        parsed.error = "shape";
        parsed.detail = "not a JSON object";
        return parsed;
    }
    const QJsonObject data = document.object();
    const QJsonValue version = data.value("contractVersion");
    parsed.contractVersion = isInteger(version) ? version.toInt() : 0;
    const QJsonValue from = data.value("contractReadableFrom");
    parsed.readableFrom = isInteger(from) && from.toInt() >= 1 && from.toInt() <= parsed.contractVersion
        ? from.toInt() : parsed.contractVersion;
    const bool reads = parsed.contractVersion == ContractVersion
        || (parsed.contractVersion > ContractVersion && parsed.readableFrom <= ContractVersion);
    if (!reads) {
        parsed.error = "contract";
        return parsed;
    }
    parsed.newer = parsed.contractVersion > ContractVersion;
    if (!data.value("summary").isObject() || !data.value("state").isObject() || !data.value("generatedAt").isString()) {
        parsed.error = "shape";
        parsed.detail = "summary, state or generatedAt missing";
        return parsed;
    }
    parsed.index = data;
    return parsed;
}

void Backend::reload() {
    // One read at a time; a change during a read asks for one more.
    if (m_reading.isRunning()) {
        m_readAgain = true;
        return;
    }
    const QString path = m_options.indexPath;
    m_reading.setFuture(QtConcurrent::run([path] { return readIndex(path); }));
}

// The engine replaces the index by a rename, which drops a watch on the file
// itself, and a first `init` creates the directory; so watch the file, its
// directory and the nearest ancestor that exists, and re-arm after every read.
void Backend::rearm() {
    const auto paths = m_watcher.files() + m_watcher.directories();
    if (!paths.isEmpty())
        m_watcher.removePaths(paths);
    const QString directory = QFileInfo(m_options.indexPath).absolutePath();
    QString ancestor = directory;
    while (!QFileInfo::exists(ancestor) && ancestor != "/")
        ancestor = QFileInfo(ancestor).absolutePath();
    QStringList candidates{ancestor, directory, m_options.indexPath};
    candidates.removeDuplicates();
    for (const auto &path : candidates)
        if (QFileInfo::exists(path))
            m_watcher.addPath(path);
}

void Backend::ingest(const Parsed &parsed) {
    rearm();
    const bool indexWas = hasIndex();
    const QJsonObject before = m_parsed.index;
    m_parsed = parsed;
    m_fileState = parsed.error.isEmpty() || parsed.error == "contract" ? "loaded"
        : parsed.error == "missing" ? "missing" : "invalid";
    // The engine writes whole seconds: an index written in the second of the
    // exit 3 is the new one (`seldon init` right after), an older one is a
    // leftover (Service.ingest).
    if (m_engineNotInitialised && hasIndex()
        && timeMs(parsed.index.value("generatedAt").toString()) >= (m_notInitialisedAtMs / 1000) * 1000)
        m_engineNotInitialised = false;
    if (indexWas != hasIndex() || before != m_parsed.index)
        emit indexChanged();
    refreshStatus();
}

// ---- Status -----------------------------------------------------------------

bool Backend::isStale(const QString &generatedAt, qint64 nowMs) {
    const qint64 at = timeMs(generatedAt);
    if (at < 0)
        return true;
    return nowMs - at > StaleAfterMs;
}

QString Backend::deriveStatus(const QString &engineState, const QString &fileState, const Parsed &parsed,
                              bool engineNotInitialised, qint64 nowMs) {
    // Without an engine nothing can be fixed, so engineMissing wins; then the
    // index file itself; then what the index says.
    if (engineState == "missing")
        return "engineMissing";
    if (fileState == "loading")
        return "loading";
    if (fileState != "loaded")
        return engineNotInitialised ? "notInitialised" : "indexMissing";
    if (parsed.error == "contract")
        return "contractMismatch";
    if (!parsed.error.isEmpty())
        return engineNotInitialised ? "notInitialised" : "indexMissing";
    const QString state = parsed.index.value("state").toObject().value("status").toString();
    if (engineNotInitialised || state == "notInitialised")
        return "notInitialised";
    if (state == "indexStale" || isStale(parsed.index.value("generatedAt").toString(), nowMs))
        return "indexStale";
    return "ok";
}

// The clock the status reads: live, or in dev mode pinned to SELDON_NOW,
// else to the index's own generatedAt, so a fixture never turns stale
// (Model.effectiveNowMs).
qint64 Backend::currentMs() const {
    const qint64 live = QDateTime::currentMSecsSinceEpoch();
    if (!m_options.devMode)
        return live;
    const qint64 pinned = timeMs(m_options.now);
    if (pinned >= 0)
        return pinned;
    const qint64 generated = timeMs(m_parsed.index.value("generatedAt").toString());
    return generated >= 0 ? generated : live;
}

void Backend::refreshStatus() {
    m_status = deriveStatus(m_engineState, m_fileState, m_parsed, m_engineNotInitialised, currentMs());
    emit stateChanged();
}

// ---- Engine calls -----------------------------------------------------------

// The forms of CONTRACT.md's "Commands the plugin may run" that this window
// uses, word for word as Model.js builds them (driftArgs and the constants).
// Free text is one argument after `--`, one line, and never part of a
// command string: QProcess passes every word as it is.
QString Backend::validateArgs(const QStringList &args) {
    if (args.isEmpty())
        return "empty argument list";
    for (const auto &arg : args)
        if (arg.contains(QChar(0)))
            return "an argument contains NUL";
    const qsizetype separator = args.indexOf("--");
    QStringList words = separator < 0 ? args : args.mid(0, separator);
    const QStringList free = separator < 0 ? QStringList() : args.mid(separator + 1);
    const bool withText = separator >= 0;
    if (withText) {
        static const QRegularExpression lineBreak(QStringLiteral("[\\r\\n]"));
        if (free.size() != 1 || free.first().trimmed().isEmpty())
            return "free text must be one non-empty argument after --";
        if (free.first().contains(lineBreak))
            return "free text must be one line";
    }
    if (words.isEmpty())
        return "no command before --";
    // CONTRACT.md writes capture with --quiet last.
    if (words.first() == "capture")
        return words == QStringList{"capture", "--all", "--json", "--quiet"} && !withText
            ? QString() : "capture must be: capture --all --json --quiet";
    if (words.size() < 2 || words.last() != "--json")
        return "every call ends in --json";
    words.removeLast();
    const qsizetype n = words.size();
    const QString head = words.first();

    if (head == "--version" || head == "status")
        return n == 1 && !withText ? QString() : head + " takes no arguments";
    if (head != "drift" || n < 3 || !EventId.match(words[2]).hasMatch())
        return "command not allowed: " + head;

    const QString action = words[1];
    qsizetype k = 3;
    if (action == "show")
        return n == 3 && !withText ? QString() : "drift show must be: drift show <eventId> --json";
    if (action == "link") {
        if (withText || n < 4 || !CaseId.match(words[3]).hasMatch())
            return "drift link must be: drift link <eventId> <caseId> [--only] --json";
        k = 4;
        if (k < n && words[k] == "--only")
            ++k;
        return k == n ? QString() : "drift link must be: drift link <eventId> <caseId> [--only] --json";
    }
    if (action == "dismiss" || action == "explain") {
        if (!withText)
            return "drift " + action + " needs its text after --";
        if (k < n && words[k] == "--only")
            ++k;
        if (action == "explain") {
            // Then optionally --zone, --risk, --area, in that order (SPEC-ENGINE §3).
            if (k + 1 < n && words[k] == "--zone" && Zones.contains(words[k + 1]))
                k += 2;
            if (k + 1 < n && words[k] == "--risk" && Risks.contains(words[k + 1]))
                k += 2;
            if (k + 1 < n && words[k] == "--area" && Area.match(words[k + 1]).hasMatch())
                k += 2;
        }
        return k == n ? QString()
            : "drift " + action + " must be: drift " + action + " <eventId> [--only]"
              + (action == "explain" ? " [--zone <z>] [--risk <r>] [--area <a>]" : "") + " --json -- <text>";
    }
    return "drift action not allowed: " + action;
}

void Backend::probe() {
    if (m_probe.state() != QProcess::NotRunning)
        return;
    m_probe.start(m_options.engine, {"--version", "--json"});
}

bool Backend::run(const QStringList &args) {
    const QString reason = validateArgs(args);
    if (!reason.isEmpty()) {
        qWarning("seldon-desk: refused engine call: %s", qPrintable(reason));
        return false;
    }
    // CONTRACT.md rule 1: dev mode only probes the version.
    if (m_options.devMode || m_engineState == "missing")
        return false;
    m_queue.append(args);
    emit busyChanged();
    startNext();
    return true;
}

void Backend::startNext() {
    if (m_process.state() != QProcess::NotRunning || m_queue.isEmpty()) {
        emit busyChanged();
        return;
    }
    m_running = m_queue.takeFirst();
    m_process.start(m_options.engine, m_running);
    m_timeout.start();
    emit busyChanged();
}

void Backend::callFinished(int exitCode, QProcess::ExitStatus exitStatus) {
    m_timeout.stop();
    const QStringList args = m_running;
    m_running.clear();
    const int code = exitStatus == QProcess::NormalExit ? exitCode : -1;
    const QString output = QString::fromUtf8(m_process.readAllStandardOutput());
    const QString errorOutput = QString::fromUtf8(m_process.readAllStandardError());
    // Exit 3: no logbook yet (AGENTS.md §7).
    if (code == 3 && (args.first() == "status" || args.first() == "capture")) {
        m_engineNotInitialised = true;
        m_notInitialisedAtMs = QDateTime::currentMSecsSinceEpoch();
    }
    emit finished(args, code, output, errorOutput);
    // A writing call rewrites the index by a rename the watcher sees; read
    // it again anyway, the status may have changed without a new file.
    reload();
    startNext();
}

bool Backend::fix() {
    if (m_status == "indexMissing") {
        m_statusAsked = true;
        return run({"status", "--json"});
    }
    if (m_status == "indexStale")
        return run({"capture", "--all", "--json", "--quiet"}) && run({"status", "--json"});
    return false;
}

bool Backend::openTerminal(const QStringList &argv) {
    if (m_options.devMode || argv.size() != 2 || argv.first() != TerminalLauncher || argv.last().trimmed().isEmpty())
        return false;
    // Opens on the user's click only; the script is one of Model.js's
    // constants (Model.terminalArgv). Look for the engine for a while after,
    // since the install runs in that terminal.
    if (!QProcess::startDetached(m_options.launcher, {argv.last()}))
        return false;
    m_repollLeft = RepollTries;
    m_repoll.start();
    return true;
}

void Backend::recheck() {
    probe();
    reload();
}
