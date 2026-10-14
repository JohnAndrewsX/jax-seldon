// Theme watching follows Omacut (MIT, David Heinemeier Hansson), through the
// omarchy-app skill's template; the palette follows Omarchy's shell
// (Commons/Color.qml, Commons/Style.qml).
#include "theme.h"

#include <QColor>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QStandardPaths>
#include <cmath>

namespace {

// The only colour literals of the app: what it shows without Omarchy. The
// accent is the omarchy-app default; the rest are the shell's own defaults
// (Commons/Color.qml), so a window without a theme looks like the shell
// without one.
const char *const FallbackAccent = "#FFD60A";
const char *const FallbackForeground = "#cacccc";
const char *const FallbackBackground = "#101315";
const char *const FallbackUrgent = "#a55555";
const char *const FallbackMuted = "#707880";

// The shell's Style fills over the foreground: normal, hover, selected, and
// the focus border (Commons/Style.qml defaults).
constexpr double NormalFillAlpha = 0.04;
constexpr double HoverFillAlpha = 0.08;
constexpr double SelectedFillAlpha = 0.18;
constexpr double FocusBorderAlpha = 0.25;

QString withAlpha(const QString &colour, double alpha) {
    QColor c(colour);
    c.setAlphaF(alpha);
    return c.name(QColor::HexArgb);
}

QString defaultDirectory() {
    const QString state = qEnvironmentVariable("XDG_STATE_HOME");
    return (state.isEmpty() ? QDir::homePath() + "/.local/state" : state) + "/omarchy/current";
}

} // namespace

Theme::Theme(const QString &directory, QObject *parent, const QString &hyprctl)
    : QObject(parent),
      m_directory(directory.isEmpty() ? defaultDirectory() : directory),
      m_palette(readPalette({})) {
    m_debounce.setSingleShot(true);
    m_debounce.setInterval(80);
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, [this] { m_debounce.start(); });
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, this, [this] { m_debounce.start(); });
    connect(&m_debounce, &QTimer::timeout, this, &Theme::reload);

    // Corners follow Hyprland's effective rounding, which themes and personal
    // overrides can both set, so ask hyprctl rather than reading its config.
    // Without Hyprland the corners stay sharp, like Omarchy's default.
    m_hyprctl = !hyprctl.isEmpty() ? hyprctl
              : directory.isEmpty() && !qEnvironmentVariableIsEmpty("HYPRLAND_INSTANCE_SIGNATURE")
                  ? QStandardPaths::findExecutable("hyprctl") : QString();
    m_roundingTimeout.setSingleShot(true);
    m_roundingTimeout.setInterval(500);
    connect(&m_roundingTimeout, &QTimer::timeout, &m_roundingProcess, &QProcess::kill);
    connect(&m_roundingProcess, &QProcess::errorOccurred, this, [this] { m_roundingTimeout.stop(); });
    connect(&m_roundingProcess, &QProcess::finished, this, [this](int code, QProcess::ExitStatus status) {
        m_roundingTimeout.stop();
        const auto value = QJsonDocument::fromJson(m_roundingProcess.readAllStandardOutput()).object().value("int");
        if (code != 0 || status != QProcess::NormalExit || !value.isDouble() || value.toInt(-1) < 0)
            return;
        if (value.toInt() != m_radius) {
            m_radius = value.toInt();
            emit changed();
        }
    });
    m_roundingPoll.setInterval(1000);
    connect(&m_roundingPoll, &QTimer::timeout, this, &Theme::refreshRounding);
    if (!m_hyprctl.isEmpty())
        m_roundingPoll.start();
    reload();
}

Theme::~Theme() {
    if (m_roundingProcess.state() != QProcess::NotRunning) {
        m_roundingProcess.kill();
        m_roundingProcess.waitForFinished(1000);
    }
}

void Theme::refreshRounding() {
    if (m_hyprctl.isEmpty() || m_roundingProcess.state() != QProcess::NotRunning)
        return;
    m_roundingProcess.start(m_hyprctl, {"-j", "getoption", "decoration:rounding"});
    m_roundingTimeout.start();
}

QString Theme::readAccent(const QString &path) {
    return readPalette(path).value("accent");
}

// The roles as the shell reads them: an explicit key first, then the
// terminal colour the shell falls back to (color0, color7, color4, color8),
// and `red` or `color1`, whichever comes last, for urgent.
QHash<QString, QString> Theme::readPalette(const QString &path) {
    QHash<QString, QString> keys;
    QFile file(path);
    const bool loaded = !path.isEmpty() && file.open(QIODevice::ReadOnly);
    if (loaded) {
        const QRegularExpression entry(R"re(^\s*([A-Za-z0-9_-]+)\s*=\s*["']?(#[0-9A-Fa-f]{6}))re");
        while (!file.atEnd()) {
            const auto match = entry.match(QString::fromUtf8(file.readLine()));
            if (!match.hasMatch())
                continue;
            const QString key = match.captured(1);
            keys.insert(key == "color1" ? "red" : key, match.captured(2));
        }
    }
    auto pick = [&](const char *key, const char *alternative, const QString &fallback) {
        return keys.value(key, keys.value(alternative, fallback));
    };
    QHash<QString, QString> palette;
    palette["foreground"] = pick("foreground", "color7", FallbackForeground);
    palette["background"] = pick("background", "color0", FallbackBackground);
    palette["accent"] = pick("accent", "color4", FallbackAccent);
    palette["urgent"] = keys.value("red", FallbackUrgent);
    palette["muted"] = pick("muted", "color8", loaded ? palette["foreground"] : QString(FallbackMuted));
    palette["normalFill"] = withAlpha(palette["foreground"], NormalFillAlpha);
    palette["hoverFill"] = withAlpha(palette["foreground"], HoverFillAlpha);
    palette["selectedFill"] = withAlpha(palette["foreground"], SelectedFillAlpha);
    palette["focusBorder"] = withAlpha(palette["foreground"], FocusBorderAlpha);
    return palette;
}

// Black or white, whichever reads on the accent (WCAG relative luminance).
QString Theme::accentForeground() const {
    const QColor c(accent());
    auto linear = [](double v) { return v <= .04045 ? v / 12.92 : std::pow((v + .055) / 1.055, 2.4); };
    const double luminance = .2126 * linear(c.redF()) + .7152 * linear(c.greenF()) + .0722 * linear(c.blueF());
    return luminance > .179 ? "black" : "white";
}

void Theme::reload() {
    refreshRounding();
    const auto paths = m_watcher.files() + m_watcher.directories();
    if (!paths.isEmpty())
        m_watcher.removePaths(paths);

    // A theme switch replaces symlinks and files, so watch the parents too and
    // re-arm after every change. Watching the nearest existing ancestor means
    // installing Omarchy later works without restarting the app.
    QString ancestor = m_directory;
    while (!QFileInfo::exists(ancestor) && ancestor != "/")
        ancestor = QFileInfo(ancestor).absolutePath();
    QStringList candidates{ancestor, QFileInfo(ancestor).absolutePath(), m_directory,
                           m_directory + "/theme", m_directory + "/theme/colors.toml"};
    candidates.removeDuplicates();
    for (const auto &path : candidates)
        if (QFileInfo::exists(path))
            m_watcher.addPath(path);

    const auto palette = readPalette(m_directory + "/theme/colors.toml");
    if (palette != m_palette) {
        m_palette = palette;
        emit changed();
    }
}
