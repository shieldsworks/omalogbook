import QtQuick
import qs.Commons

// What the log adds up to: the distance sailed, the time under way, the
// speed that makes, and the days worth remembering. Escape closes.
//
// Everything here is read from the notes, so a total the crew corrected by
// hand is the total shown. Nothing is kept anywhere else to disagree with
// them.
Item {
    id: root
    focus: true

    signal closeRequested
    Keys.onPressed: e => {
        if (e.key === Qt.Key_Escape) root.closeRequested();
        else if (e.key === Qt.Key_R) Totals.refresh();
        else return;
        e.accepted = true;
    }

    readonly property color ink: Color.foreground
    // Secondary lines are the text color, faded. The theme's muted color is
    // too dark to read on the popover.
    readonly property real faint: 0.65

    readonly property var all: Totals.all
    // Guarded on this object's own copy of the totals, not on
    // Totals.anything: when the vault has just been read, a binding here
    // can run before `all` has caught up, and would read a figure off
    // nothing.
    readonly property bool any: !!root.all && Totals.num(root.all.days) > 0
    readonly property var year: Totals.year
    readonly property var today: Totals.today
    readonly property real average: Totals.num(root.all && root.all.averageKn)

    function nm(v) {
        return Totals.num(v).toFixed(1) + " nm";
    }
    // A record, with the day it was set. Nothing is shown for a record no
    // day has set yet.
    function record(of, text) {
        const v = Totals.value(of);
        return v === null ? "" : text(v) + " · " + Totals.on(of);
    }

    component Line: Text {
        color: root.ink
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
    }

    // Label left, figure right, as a ledger is ruled.
    component Figure: Item {
        id: row
        property string label: ""
        property string figure: ""
        property bool strong: false
        width: parent ? parent.width : 0
        height: visible ? Math.max(name.implicitHeight, value.implicitHeight) : 0
        visible: figure !== ""
        Line {
            id: name
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width * 0.42
            text: row.label
            opacity: root.faint
        }
        Line {
            id: value
            anchors.right: parent.right
            anchors.left: name.right
            anchors.verticalCenter: parent.verticalCenter
            text: row.figure
            horizontalAlignment: Text.AlignRight
            font.bold: row.strong
        }
    }

    component Gap: Item {
        width: 1
        height: Style.space(8)
    }

    Column {
        id: sheet
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: 3

        Line {
            width: parent.width
            text: Totals.boat !== "" ? Totals.boat : "The log"
            font.pixelSize: Style.font.body
            font.bold: true
        }
        Line {
            width: parent.width
            visible: text !== ""
            opacity: root.faint
            text: {
                if (!Totals.read) return "Reading the log…";
                if (Totals.error !== "") return "";
                if (!root.any) return "No days logged yet";
                if (Totals.first === "") return Totals.num(root.all.days) + " days logged, none sailed yet";
                const span = Totals.num(Totals.spanDays);
                return Totals.first + " to " + Totals.last + (span > 0 ? " · " + span + " days" : "");
            }
        }
        Line {
            width: parent.width
            visible: Totals.error !== ""
            text: Totals.error
            color: Color.urgent
            wrapMode: Text.WordWrap
            maximumLineCount: 4
            elide: Text.ElideRight
        }

        Gap {}

        // The one number a sailor keeps: how far the boat has gone.
        Line {
            width: parent.width
            visible: root.any
            text: root.nm(Totals.distanceNm)
            font.pixelSize: Style.font.display
            font.bold: true
        }
        Line {
            width: parent.width
            visible: root.any
            opacity: root.faint
            text: {
                const sailed = Totals.num(root.all && root.all.daysSailed);
                return sailed === 1 ? "on 1 day sailed" : "on " + sailed + " days sailed";
            }
        }

        Gap {}

        Figure {
            label: "Under way"
            figure: root.any ? Totals.hours(root.all.underwaySecs) : ""
        }
        Figure {
            label: "Average"
            figure: root.average > 0 ? root.average.toFixed(1) + " kn" : ""
        }
        Figure {
            label: "Fastest"
            figure: root.record(Totals.fastest, v => v.toFixed(1) + " kn")
        }
        Figure {
            label: "Passages"
            figure: root.any ? String(Totals.num(root.all.passages)) : ""
        }

        Gap {}

        Figure {
            label: "Biggest day"
            figure: root.record(Totals.bestDay, v => root.nm(v))
        }
        Figure {
            label: "Longest day"
            figure: root.record(Totals.longestDay, () => Totals.hours(Totals.longestDay.secs))
        }
        Gap { visible: seasonRow.visible }

        // The season so far, worth its own line once the log is older than
        // this year.
        Figure {
            id: seasonRow
            label: String(Totals.thisYear)
            figure: {
                if (!root.year || !root.all || Totals.num(root.year.days) === 0) return "";
                if (Totals.num(root.year.days) >= Totals.num(root.all.days)) return "";
                return root.nm(root.year.distanceNm) + " · " + Totals.hours(root.year.underwaySecs);
            }
        }

        Gap { visible: todayRow.visible }

        // Today, while it is still happening. This is the figure that moves
        // as you watch it.
        Figure {
            id: todayRow
            label: "Today"
            strong: true
            figure: {
                const d = root.today;
                if (!d) return "";
                const nm = Totals.num(d.distanceNm), secs = Totals.num(d.underwaySecs);
                if (nm <= 0 && secs <= 0) return "";
                return root.nm(nm) + " · " + Totals.hours(secs);
            }
        }
    }

    Line {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        opacity: root.faint * 0.8
        visible: !root.any && Totals.read && Totals.error === ""
        wrapMode: Text.WordWrap
        text: "The log fills as you sail: `omalogbook run` keeps the watch."
    }
}
