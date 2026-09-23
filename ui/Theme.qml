import QtQuick
import Quickshell
import Quickshell.Io

// The Omarchy theme: its colors.toml, and the shell's font size. Reloads
// when the theme changes, so the window follows a theme switch at once.
// This is for the window, which runs without the shell and so has no
// qs.Commons to read. As Omalookout's and Omawind's do.
QtObject {
    id: theme
    readonly property string dir: Quickshell.env("OMALOGBOOK_THEME_DIR")
        || Quickshell.env("HOME") + "/.local/state/omarchy/current/theme"
    readonly property string shellConfig: Quickshell.env("HOME") + "/.config/omarchy/shell.toml"
    property var colors: ({})
    property var shellValues: ({})
    // Night Watch: red on black whatever the theme, to keep night vision.
    // Off at every start; this window's own, not the desktop's.
    property bool night: false
    readonly property var nightWatch: ({
        background: "#0c0404", foreground: "#e8503f", accent: "#ff3b2f",
        red: "#ff3b2f", yellow: "#ffa28a"
    })

    // `key = "value"` and `key = 12` lines, keyed section.key. Enough for
    // colors.toml and shell.toml; not a general TOML reader.
    function read(text) {
        var out = {}, section = "";
        var lines = String(text).split("\n");
        for (var i = 0; i < lines.length; i++) {
            var line = lines[i].trim();
            if (!line || line[0] === "#") continue;
            var head = line.match(/^\[([^\]]+)\]\s*(#.*)?$/);
            if (head) { section = head[1].trim() + "."; continue; }
            // A quoted value keeps its `#` (colors are "#rrggbb"); a bare
            // one ends at a comment.
            var kv = line.match(/^([\w.-]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^#]*?))\s*(#.*)?$/);
            if (!kv) continue;
            var v = kv[2] !== undefined ? kv[2] : kv[3] !== undefined ? kv[3] : kv[4];
            if (kv[4] !== undefined && /^-?\d+(\.\d+)?$/.test(v)) v = Number(v);
            out[section + kv[1]] = v;
        }
        return out;
    }
    function color(key, fallback) {
        var v = night ? nightWatch[key] : colors[key];
        return typeof v === "string" && /^#[0-9a-fA-F]{6}$/.test(v) ? v : fallback;
    }

    readonly property color background: color("background", "#1a1b26")
    readonly property color foreground: color("foreground", "#a9b1d6")
    readonly property color accent: color("accent", "#7aa2f7")
    readonly property color red: color("red", "#f7768e")
    readonly property color yellow: color("yellow", "#e0af68")
    readonly property string font: "monospace"
    readonly property int baseSize: {
        var n = Number(shellValues["font.base-size"]);
        return n > 0 && n < 40 ? n : 12;
    }

    // `omarchy theme set` does not rewrite the theme's files. It deletes the
    // whole `current/theme` directory and moves a new one into its place, so
    // for as long as that takes there is no colors.toml to read and the watch
    // is on a directory that has gone. A read landing in that gap fails, and a
    // failure taken as final leaves the window on the fallbacks above — which
    // are Tokyo Night's, and so look exactly like a theme switch that didn't
    // happen.
    //
    // Two things keep that from sticking. A failed read is retried a few times
    // rather than believed, which covers the swap itself; and a slow re-read
    // catches a swap whose watch went with the directory it was watching. The
    // colors already in hand are kept through both, so a theme change is a
    // change of palette rather than a flash through the fallbacks.
    property int attempt: 0
    readonly property int attempts: 5

    property Timer retry: Timer {
        interval: 500
        repeat: false
        onTriggered: theme.colorsFile.reload()
    }
    // Long enough to cost nothing, short enough that a lost watch is a
    // nuisance rather than a reason to restart the window.
    property Timer resettle: Timer {
        interval: 30000
        repeat: true
        running: true
        onTriggered: theme.colorsFile.reload()
    }

    property FileView colorsFile: FileView {
        path: theme.dir + "/colors.toml"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            theme.colors = theme.read(text());
            theme.attempt = 0;
        }
        // Bounded, so a theme directory that is genuinely gone settles down
        // to the slow re-read instead of spinning on it.
        onLoadFailed: {
            if (theme.attempt >= theme.attempts) return;
            theme.attempt++;
            theme.retry.restart();
        }
    }
    property FileView shellFile: FileView {
        path: theme.shellConfig
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: theme.shellValues = theme.read(text())
        onLoadFailed: theme.shellValues = ({})
    }
}
