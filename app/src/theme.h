#pragma once

#include <QFileSystemWatcher>
#include <QHash>
#include <QObject>
#include <QProcess>
#include <QTimer>

// Follows the Omarchy theme live: the accent of the omarchy-app template plus
// the roles Omarchy's shell derives from colors.toml (Commons/Color.qml), which
// the desk's tones (plugin/Tones.js, Model.deskTones) need. The fills are the
// shell's Style defaults over the foreground. Colours are "#rrggbb", or Qt's
// "#aarrggbb" for the translucent fills.
class Theme : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString accent READ accent NOTIFY changed)
    Q_PROPERTY(QString accentForeground READ accentForeground NOTIFY changed)
    Q_PROPERTY(QString background READ background NOTIFY changed)
    Q_PROPERTY(QString foreground READ foreground NOTIFY changed)
    Q_PROPERTY(QString urgent READ urgent NOTIFY changed)
    Q_PROPERTY(QString muted READ muted NOTIFY changed)
    Q_PROPERTY(QString normalFill READ normalFill NOTIFY changed)
    Q_PROPERTY(QString hoverFill READ hoverFill NOTIFY changed)
    Q_PROPERTY(QString selectedFill READ selectedFill NOTIFY changed)
    Q_PROPERTY(QString focusBorder READ focusBorder NOTIFY changed)
    Q_PROPERTY(int focusBorderWidth READ focusBorderWidth CONSTANT)
    Q_PROPERTY(int radius READ radius NOTIFY changed)

public:
    explicit Theme(const QString &currentDirectory = {}, QObject *parent = nullptr, const QString &hyprctl = {});
    ~Theme() override;

    QString accent() const { return m_palette.value("accent"); }
    QString accentForeground() const;
    QString background() const { return m_palette.value("background"); }
    QString foreground() const { return m_palette.value("foreground"); }
    QString urgent() const { return m_palette.value("urgent"); }
    QString muted() const { return m_palette.value("muted"); }
    QString normalFill() const { return m_palette.value("normalFill"); }
    QString hoverFill() const { return m_palette.value("hoverFill"); }
    QString selectedFill() const { return m_palette.value("selectedFill"); }
    QString focusBorder() const { return m_palette.value("focusBorder"); }
    int focusBorderWidth() const { return 1; }
    int radius() const { return m_radius; }

    static QString readAccent(const QString &path);
    // Every role above from one colors.toml, with the fallbacks for a missing
    // file or key.
    static QHash<QString, QString> readPalette(const QString &path);

signals:
    void changed();

private:
    void reload();
    void refreshRounding();

    QString m_directory;
    QHash<QString, QString> m_palette;
    QFileSystemWatcher m_watcher;
    QTimer m_debounce;
    QString m_hyprctl;
    int m_radius = 0;
    QProcess m_roundingProcess;
    QTimer m_roundingPoll, m_roundingTimeout;
};
