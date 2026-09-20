import QtQuick
import Quickshell
import Quickshell.Io

// The ship's log in a window of its own: a line to write in, the day's
// entries under it, and the day's run at the foot. Run standalone
// (ui/shell.qml) it owns its process; as the shell's panel the shell opens
// and hides it.
//
// Writing is the point. The field keeps the focus, so a note on watch is
// the words and Enter — no button to find with a tiller under your arm.
Item {
    id: app

    // Set by the Omarchy shell when loaded as a panel.
    property var shell: null
    property var manifest: null
    property bool standalone: true
    property bool opened: standalone

    function open(payload) {
        opened = true;
        Qt.callLater(() => field.forceActiveFocus());
    }
    function close() {
        opened = false;
    }
    function dismiss() {
        if (standalone) Qt.quit();
        else if (shell) shell.hide("org.omahoy.logbook");
        else opened = false;
    }

    property Theme theme: Theme {}
    property Book book: Book {}

    // Quickshell keeps a process alive after its last window closes.
    Connections {
        target: Quickshell
        function onLastWindowClosed() { if (app.standalone) Qt.quit(); }
    }

    property string toastText: ""
    function toast(text) {
        toastText = text;
        toastTimer.restart();
    }
    Timer { id: toastTimer; interval: 3500; onTriggered: app.toastText = "" }

    Connections {
        target: app.book
        // omalogbook files the entry either way; when it had something to
        // say about it — a slash word that wasn't a mark — say that instead
        // of "Logged."
        function onFiled(said) { app.toast(said !== "" ? said : "Logged."); }
        function onRefused(message) { app.toast(message); }
    }

    // The marks the line could still become. Empty unless the line opens
    // with a slash, so prose never has a list hanging under it.
    readonly property var matches: {
        var text = field.text;
        if (text.length === 0 || text[0] !== "/") return [];
        var typed = text.slice(1).split(/\s/)[0].toLowerCase();
        // Once a mark is named and the crew has moved on to the words after
        // it, the list has done its job.
        if (text.slice(1).indexOf(" ") >= 0) return [];
        return app.book.presets.filter(p => String(p.word).indexOf(typed) === 0);
    }

    // Tab takes the first one, so a mark is two or three keys.
    function complete() {
        if (app.matches.length === 0) return;
        field.text = "/" + app.matches[0].word + " ";
        field.cursorPosition = field.text.length;
    }

    // The entry being changed, or -1 for a new one. The line as it was read
    // goes back with the change, so an entry the running log has shifted
    // under us is refused rather than mistaken for another.
    property int editing: -1
    property string editingLine: ""

    function edit(index) {
        if (index < 0 || index >= app.book.entries.length) return;
        app.editing = index;
        app.editingLine = String(app.book.entries[index]);
        var said = app.editingLine.indexOf(" — ");
        field.text = said >= 0 ? app.editingLine.slice(said + 3) : "";
        field.cursorPosition = field.text.length;
        field.forceActiveFocus();
    }

    function cancel() {
        app.editing = -1;
        app.editingLine = "";
        field.text = "";
    }

    function file() {
        var words = field.text.trim();
        if (words === "") return;
        if (app.book.filing) {
            app.toast("Still filing the last one.");
            return;
        }
        if (app.editing >= 0) {
            if (app.book.amend(app.editing, app.editingLine, words)) app.cancel();
            return;
        }
        if (app.book.note(words)) field.text = "";
    }

    function strikeEditing() {
        if (app.editing < 0 || app.book.filing) return;
        if (app.book.strike(app.editing, app.editingLine)) app.cancel();
    }

    // A struck entry keeps its line through it here too, rather than showing
    // the tildes the file uses to say so.
    function struck(line) {
        return String(line).indexOf("~~") >= 0;
    }

    // The day as a line: `12.4 nm · fastest 6.1 kn · under way 3 h 12 min`.
    readonly property string totals: {
        if (!app.book.read || app.book.entries.length === 0) return "";
        var secs = app.book.underwaySecs;
        var hours = Math.floor(secs / 3600), mins = Math.floor(secs % 3600 / 60);
        var underway = hours > 0 ? hours + " h " + (mins < 10 ? "0" : "") + mins + " min" : mins + " min";
        return app.book.distanceNm.toFixed(1) + " nm  ·  fastest " + app.book.maxSogKn.toFixed(1)
            + " kn  ·  under way " + underway;
    }

    readonly property string emptyText: {
        if (app.book.error !== "") return app.book.error;
        if (!app.book.read || app.book.entries.length > 0) return "";
        return "Nothing logged today.\n\nWrite a note above, or start the log with\nomalogbook run to keep the watch.";
    }

    // An entry without its markdown: `- **09:15** (16:15 UTC) · …` is for
    // the file, not for a window.
    function plain(line) {
        return String(line).replace(/^-\s+/, "").replace(/\*\*/g, "").replace(/~~/g, "");
    }

    // For checks, where no keyboard can be driven:
    //   quickshell ipc -p ui/shell.qml call omalogbook status
    IpcHandler {
        target: "omalogbook"
        function status(): string {
            return JSON.stringify({date: app.book.date, boat: app.book.boat, entries: app.book.entries.length,
                                   read: app.book.read, error: app.book.error, filing: app.book.filing,
                                   opened: app.opened, night: app.theme.night});
        }
        function night(): void { app.theme.night = !app.theme.night; }
        function note(text: string): void { app.book.note(text); }
        // Put words in the field without a keyboard, for checks.
        function typed(text: string): void {
            field.text = text;
            field.cursorPosition = field.text.length;
        }
        function complete(): void { app.complete(); }
        function file(): void { app.file(); }
        function edit(index: int): void { app.edit(index); }
        function cancel(): void { app.cancel(); }
        function strike(): void { app.strikeEditing(); }
        function editingAt(): int { return app.editing; }
        function marks(): string { return JSON.stringify(app.matches.map(m => m.word)); }
        function refresh(): void { app.book.refresh(); }
    }

    FloatingWindow {
        id: win
        title: "Omalogbook"
        visible: app.opened
        onVisibleChanged: {
            if (!visible && app.opened) app.dismiss();
            else if (visible) Qt.callLater(() => field.forceActiveFocus());
        }
        implicitWidth: Number(Quickshell.env("OMALOGBOOK_WIDTH")) || 460
        implicitHeight: Number(Quickshell.env("OMALOGBOOK_HEIGHT")) || 700
        color: app.theme.background

        // Plain text: an entry carries what the crew wrote, and that
        // mustn't be read as markup.
        component Label: Text {
            textFormat: Text.PlainText
            color: app.theme.foreground
            font.family: app.theme.font
            font.pixelSize: app.theme.baseSize
            elide: Text.ElideRight
        }

        Item {
            id: surface
            anchors.fill: parent

            // The app's name, so the window is known at a glance.
            Label {
                id: appName
                anchors { left: parent.left; right: nightButton.left; top: parent.top; margins: 14 }
                text: "OMALOGBOOK"
                color: app.theme.accent
                font.bold: true
                font.pixelSize: app.theme.baseSize - 1
            }

            // Night Watch for this window only: red on black.
            Rectangle {
                id: nightButton
                anchors { right: parent.right; rightMargin: 14; verticalCenter: appName.verticalCenter }
                height: 22
                width: nightLabel.implicitWidth + 16
                color: app.theme.night ? app.theme.accent : "transparent"
                border.width: 1
                border.color: app.theme.night ? app.theme.accent : Qt.alpha(app.theme.foreground, 0.25)
                Label {
                    id: nightLabel
                    anchors.centerIn: parent
                    text: "NIGHT  n"
                    color: app.theme.night ? app.theme.background : app.theme.foreground
                    font.pixelSize: app.theme.baseSize - 1
                }
                MouseArea { anchors.fill: parent; onClicked: app.theme.night = !app.theme.night }
            }

            Label {
                id: title
                anchors { left: parent.left; right: parent.right; top: appName.bottom; topMargin: 6; leftMargin: 14; rightMargin: 14 }
                text: app.book.date === "" ? "" : app.book.date + (app.book.boat === "" ? "" : "  ·  " + app.book.boat)
                font.pixelSize: app.theme.baseSize + 2
                font.bold: true
            }

            Rectangle {
                id: rule
                anchors { left: parent.left; right: parent.right; top: title.bottom; topMargin: 10; leftMargin: 14; rightMargin: 14 }
                height: 1
                color: app.theme.foreground
                opacity: 0.2
            }

            // A line to write in. It holds the focus whenever the window
            // has it, so `n` for Night Watch would be a letter in a note:
            // the palette key belongs to the button beside it, and Escape
            // is how you leave.
            Rectangle {
                id: strikeButton
                visible: app.editing >= 0
                anchors { right: parent.right; rightMargin: 14; verticalCenter: box.verticalCenter }
                height: box.height
                width: strikeLabel.implicitWidth + 20
                color: "transparent"
                border.width: 1
                border.color: Qt.alpha(app.theme.red, 0.6)
                Label {
                    id: strikeLabel
                    anchors.centerIn: parent
                    text: "STRIKE"
                    color: app.theme.red
                    font.pixelSize: app.theme.baseSize - 1
                }
                MouseArea { anchors.fill: parent; onClicked: app.strikeEditing() }
            }

            Rectangle {
                id: box
                anchors { left: parent.left; right: app.editing >= 0 ? strikeButton.left : parent.right; top: rule.bottom; topMargin: 12; leftMargin: 14; rightMargin: app.editing >= 0 ? 8 : 14 }
                height: app.theme.baseSize + 20
                color: Qt.alpha(app.theme.foreground, field.activeFocus ? 0.08 : 0.05)
                border.width: 1
                border.color: field.activeFocus ? app.theme.accent : Qt.alpha(app.theme.foreground, 0.25)

                TextInput {
                    id: field
                    anchors { fill: parent; leftMargin: 10; rightMargin: 10 }
                    verticalAlignment: TextInput.AlignVCenter
                    color: app.theme.foreground
                    font.family: app.theme.font
                    font.pixelSize: app.theme.baseSize
                    selectByMouse: true
                    selectionColor: app.theme.accent
                    selectedTextColor: app.theme.background
                    // An entry is one line of a log, not an essay.
                    maximumLength: 500
                    enabled: !app.book.filing
                    onAccepted: app.file()
                    Keys.onTabPressed: app.complete()
                    Keys.onEscapePressed: {
                        if (app.editing >= 0) app.cancel();
                        else if (text !== "") text = "";
                        else app.dismiss();
                    }

                    Label {
                        anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                        visible: field.text === ""
                        text: app.editing >= 0 ? "the words, then Enter" : "a note, then Enter"
                        color: Qt.alpha(app.theme.foreground, 0.45)
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.NoButton
                    cursorShape: Qt.IBeamCursor
                }
            }

            // What the line could be. It sits over the day rather than
            // pushing it down, so the entries don't jump while typing.
            Rectangle {
                id: marks
                visible: app.matches.length > 0
                z: 20
                anchors { left: box.left; right: box.right; top: box.bottom; topMargin: 4 }
                height: marksColumn.implicitHeight + 12
                // Opaque: the day showing through a list of marks is just
                // two things to read at once.
                color: app.theme.background
                border.width: 1
                border.color: Qt.alpha(app.theme.foreground, 0.25)

                Column {
                    id: marksColumn
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: 6 }
                    Repeater {
                        model: app.matches
                        Item {
                            required property var modelData
                            required property int index
                            width: marksColumn.width
                            height: app.theme.baseSize + 10
                            Label {
                                id: markWord
                                anchors { left: parent.left; leftMargin: 4; verticalCenter: parent.verticalCenter }
                                text: "/" + modelData.word
                                color: index === 0 ? app.theme.accent : app.theme.foreground
                                font.bold: index === 0
                            }
                            Label {
                                anchors { left: markWord.right; leftMargin: 10; right: parent.right; verticalCenter: parent.verticalCenter }
                                text: modelData.about
                                color: Qt.alpha(app.theme.foreground, 0.6)
                            }
                            MouseArea {
                                anchors.fill: parent
                                onClicked: {
                                    field.text = "/" + modelData.word + " ";
                                    field.cursorPosition = field.text.length;
                                    field.forceActiveFocus();
                                }
                            }
                        }
                    }
                    Label {
                        width: marksColumn.width
                        topPadding: 4
                        text: "Tab takes the first"
                        color: Qt.alpha(app.theme.foreground, 0.45)
                        font.pixelSize: app.theme.baseSize - 2
                    }
                }
            }

            // The day, oldest first, as a log reads.
            Flickable {
                id: page
                anchors { left: parent.left; right: parent.right; top: box.bottom; bottom: statusBar.top; topMargin: 12 }
                contentHeight: body.implicitHeight + 16
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                // A new entry arrives at the bottom; follow it there unless
                // the crew has scrolled back to read.
                onContentHeightChanged: if (atYEnd || contentHeight <= height) Qt.callLater(() => page.contentY = Math.max(0, contentHeight - height))

                Column {
                    id: body
                    x: 14
                    width: page.width - 28
                    spacing: 8

                    Label {
                        width: parent.width
                        visible: text !== ""
                        text: app.emptyText
                        color: app.book.error !== "" ? app.theme.red : Qt.alpha(app.theme.foreground, 0.65)
                        wrapMode: Text.Wrap
                        elide: Text.ElideNone
                        maximumLineCount: 10
                    }

                    Repeater {
                        model: app.book.entries
                        Item {
                            required property string modelData
                            required property int index
                            width: body.width
                            height: entryLabel.implicitHeight + 8

                            Rectangle {
                                anchors.fill: parent
                                anchors.margins: -4
                                visible: app.editing === index
                                color: Qt.alpha(app.theme.accent, 0.12)
                                border.width: 1
                                border.color: Qt.alpha(app.theme.accent, 0.5)
                            }
                            Label {
                                id: entryLabel
                                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                                text: app.plain(modelData)
                                // Struck through here as it is in the file:
                                // it happened, and it was withdrawn.
                                font.strikeout: app.struck(modelData)
                                color: app.struck(modelData) ? Qt.alpha(app.theme.foreground, 0.5) : app.theme.foreground
                                wrapMode: Text.Wrap
                                elide: Text.ElideNone
                                maximumLineCount: 6
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: app.editing === index ? app.cancel() : app.edit(index)
                            }
                        }
                    }
                }
            }

            Rectangle {
                id: toast
                visible: app.toastText !== ""
                anchors { left: parent.left; right: parent.right; bottom: statusBar.top; margins: 14 }
                height: toastLabel.implicitHeight + 16
                color: Qt.alpha(app.theme.background, 0.95)
                border.width: 1
                border.color: Qt.alpha(app.theme.foreground, 0.25)
                Label {
                    id: toastLabel
                    anchors { fill: parent; margins: 8 }
                    text: app.toastText
                    wrapMode: Text.Wrap
                    elide: Text.ElideRight
                    maximumLineCount: 3
                }
            }

            Rectangle {
                id: statusBar
                anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                height: app.theme.baseSize + 16
                color: app.theme.background
                Rectangle { anchors { left: parent.left; right: parent.right; top: parent.top } height: 1; color: Qt.alpha(app.theme.foreground, 0.18) }
                Label {
                    anchors { left: parent.left; leftMargin: 14; verticalCenter: parent.verticalCenter }
                    width: parent.width - 28
                    text: app.totals !== "" ? app.totals : "Nothing under way"
                    opacity: 0.65
                }
            }
        }
    }
}
