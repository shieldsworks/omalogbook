pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Every day in the log, added up, through `omalogbook totals --json`.
// A singleton, so the bar on a second monitor reads the vault once rather
// than twice.
//
// There is no socket here for the same reason there is none in Book: the
// log is a folder of text, and the notes on disk are the whole truth. The
// vault is read again when today's note changes — which is the running log
// writing its entries — and on a slow sweep, which carries the totals over
// midnight when the day's note becomes a different file.
QtObject {
    id: totals

    readonly property int version: 1
    // This file is <repo>/ui/Totals.qml, and the plugin directory is a
    // symlink to the checkout, so the build next door is found either way.
    // Installed from a release there is no build there, and every command
    // falls back to `omalogbook` on PATH, which is what `cargo install`
    // leaves. Set OMALOGBOOK_BIN to name one directly.
    readonly property string repo: decodeURIComponent(String(Qt.resolvedUrl("..")).replace(/^file:\/\//, "")).replace(/\/$/, "")
    readonly property string binary: Quickshell.env("OMALOGBOOK_BIN") || repo + "/target/release/omalogbook"

    // argv, never shell text built from paths.
    function run(args) {
        return ["sh", "-c", 'b="$1"; shift; [ -x "$b" ] || b=omalogbook; exec "$b" "$@"',
                "omalogbook-totals", totals.binary].concat(args);
    }

    // The last good read. Kept through a failed one, so a vault that is
    // briefly unreadable — a note being renamed into place — doesn't blank
    // the bar.
    property var state: null
    property bool read: false
    property string error: ""

    readonly property string boat: state && typeof state.boat === "string" ? state.boat : ""
    readonly property var all: state ? state.all : null
    readonly property var year: state ? state.year : null
    readonly property var today: state ? state.today : null
    readonly property var fastest: state ? state.fastest : null
    readonly property var bestDay: state ? state.bestDay : null
    readonly property var longestDay: state ? state.longestDay : null
    readonly property int thisYear: state && typeof state.thisYear === "number" ? state.thisYear : 0
    readonly property string first: state && typeof state.first === "string" ? state.first : ""
    readonly property string last: state && typeof state.last === "string" ? state.last : ""
    readonly property var spanDays: state ? state.spanDays : null
    readonly property string todayPath: state && typeof state.todayPath === "string" ? state.todayPath : ""

    readonly property real distanceNm: num(all && all.distanceNm)
    readonly property bool anything: !!all && num(all.days) > 0

    // Nothing read off disk is trusted to be the right shape: a note the
    // crew edited by hand must not be able to break the bar.
    function num(v) {
        return typeof v === "number" && isFinite(v) ? v : 0;
    }
    function value(of) {
        // A record that isn't set reads null, not zero: no boat has ever
        // done nought knots.
        return of && typeof of.value === "number" && isFinite(of.value) ? of.value : null;
    }
    function on(of) {
        return of && typeof of.date === "string" ? of.date : "";
    }

    // `27 h 30 min`, as the log writes it in the note.
    function hours(secs) {
        const s = Math.max(0, Math.round(num(secs)));
        const h = Math.floor(s / 3600), m = Math.floor(s % 3600 / 60);
        return h > 0 ? h + " h " + String(m).padStart(2, "0") + " min" : m + " min";
    }

    function refresh() {
        if (reader.running) return;
        reader.running = true;
    }

    function take(text) {
        var m;
        try {
            m = JSON.parse(text);
        } catch (e) {
            totals.error = "omalogbook totals --json said something this widget can't read.";
            return;
        }
        if (m === null || typeof m !== "object" || m.v !== totals.version) {
            totals.error = "omalogbook speaks version " + (m && m.v)
                + " and this widget speaks " + totals.version + ".";
            return;
        }
        totals.state = m;
        totals.error = "";
        totals.read = true;
    }

    property Process reader: Process {
        command: totals.run(["totals", "--json"])
        running: true
        stdout: StdioCollector {
            onStreamFinished: totals.take(text)
        }
        stderr: StdioCollector { id: readerErr }
        onExited: code => {
            if (code !== 0) {
                totals.error = readerErr.text.trim()
                    || ("omalogbook totals exited " + code + ".\nTried: " + totals.binary);
            }
        }
    }

    // Today's note, so the day's run reaches the bar as the running log
    // writes it, rather than at the next sweep.
    property FileView file: FileView {
        path: totals.todayPath
        watchChanges: path !== ""
        printErrors: false
        onFileChanged: totals.refresh()
    }

    // Slow, because the whole vault is read: the day's note covers the
    // minute-to-minute, and this is here for midnight and for a note
    // edited in another folder.
    property Timer sweep: Timer {
        interval: 300000
        repeat: true
        running: true
        onTriggered: totals.refresh()
    }
}
