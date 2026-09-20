import QtQuick
import Quickshell
import Quickshell.Io

// The day's log, read through omalogbook itself: `today --json` for what
// the note says, `note` to add a line. There is no socket to subscribe to
// — `omalogbook run` is a client of omakeel's, not a server — and there
// doesn't need to be: the log is a folder of text, and the file on disk is
// the whole truth. A day is reread when the note changes under us, which
// covers the running log writing its hourly entries.
QtObject {
    id: book

    readonly property int version: 1
    // Running standalone, shellDir is <repo>/ui, so the checkout's own build
    // is next door. Loaded as the shell's panel it is the shell's directory
    // instead, and the build won't be there — every command falls back to
    // `omalogbook` on PATH, which is what `cargo install` leaves. Set
    // OMALOGBOOK_BIN to name one directly.
    readonly property string binary: Quickshell.env("OMALOGBOOK_BIN")
        || Quickshell.shellDir + "/../target/release/omalogbook"

    // argv, never shell text built from paths.
    function run(args) {
        return ["sh", "-c", 'b="$1"; shift; [ -x "$b" ] || b=omalogbook; exec "$b" "$@"',
                "omalogbook-ui", book.binary].concat(args);
    }

    // The marks, read once: they only change when omalogbook does.
    property var presets: []

    property string date: ""
    property string boat: ""
    property string path: ""
    property var entries: []
    property real distanceNm: 0
    property real maxSogKn: 0
    property int underwaySecs: 0
    property var tracks: []
    // Empty until a read has failed; then what went wrong, for the window
    // to show instead of an empty day.
    property string error: ""
    property bool read: false
    // A note being filed. Two at once would race for the day's lock, so the
    // field waits rather than queues.
    property bool filing: false

    signal filed(string text)
    signal refused(string message)

    function refresh() {
        if (reader.running) return;
        reader.running = true;
    }

    // The crew's own entry. omalogbook stamps it with the time and, when
    // omakeel has a fix, the position — this window never has to know
    // where the boat is.
    function note(text) {
        var words = String(text).trim();
        if (words === "" || filing) return false;
        filing = true;
        writer.command = run(["note", words]);
        writer.running = true;
        return true;
    }

    function take(text) {
        var m;
        try {
            m = JSON.parse(text);
        } catch (e) {
            book.error = "omalogbook today --json said something this window can't read.";
            return;
        }
        if (m === null || typeof m !== "object" || m.v !== book.version) {
            book.error = "omalogbook speaks version " + (m && m.v) + " and this window speaks " + book.version + ".";
            return;
        }
        book.date = String(m.date || "");
        book.boat = String(m.boat || "");
        book.path = String(m.path || "");
        book.entries = Array.isArray(m.entries) ? m.entries : [];
        book.distanceNm = Number(m.distanceNm) || 0;
        book.maxSogKn = Number(m.maxSogKn) || 0;
        book.underwaySecs = Number(m.underwaySecs) || 0;
        book.tracks = Array.isArray(m.tracks) ? m.tracks : [];
        book.error = "";
        book.read = true;
    }

    property Process presetReader: Process {
        command: book.run(["presets", "--json"])
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    var m = JSON.parse(text);
                    if (m && m.v === book.version && Array.isArray(m.presets)) book.presets = m.presets;
                } catch (e) {
                    // An older omalogbook without `presets` costs the
                    // completion list, nothing else: the words still file.
                }
            }
        }
    }

    property Process reader: Process {
        command: book.run(["today", "--json"])
        running: true
        stdout: StdioCollector {
            onStreamFinished: book.take(text)
        }
        stderr: StdioCollector { id: readerErr }
        onExited: code => {
            if (code !== 0) {
                book.error = readerErr.text.trim() || ("omalogbook today exited " + code + ".\nTried: " + book.binary);
            }
        }
    }

    property Process writer: Process {
        command: ["true"]
        stderr: StdioCollector { id: writerErr }
        onExited: code => {
            book.filing = false;
            if (code === 0) {
                book.filed(writerErr.text.trim());
                book.refresh();
            } else {
                book.refused(writerErr.text.trim() || ("omalogbook note exited " + code + "."));
            }
        }
    }

    // The note on disk, so an entry written by the running log — or by the
    // crew in an editor — shows without waiting for the next sweep.
    property FileView file: FileView {
        path: book.path
        watchChanges: path !== ""
        printErrors: false
        onFileChanged: book.refresh()
    }

    // Midnight turns the day over, and the running log writes an entry an
    // hour: a slow sweep catches both without watching a file that may not
    // exist yet.
    property Timer sweep: Timer {
        interval: 60000
        repeat: true
        running: true
        onTriggered: book.refresh()
    }
}
