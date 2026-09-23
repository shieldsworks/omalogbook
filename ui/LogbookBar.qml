import QtQuick
import Quickshell
import qs.Ui
import qs.Commons

// The log reading in the bar — `LOG 126.9 nm`, every mile the boat has
// sailed since the first note. Click for the rest: time under way, the
// speed that makes, and the days worth remembering.
//
// The name is the instrument's: a log is what measures distance run, and
// this is that dial with the whole book behind it.
BarWidget {
    id: root
    moduleName: "org.omahoy.logbook"

    // The shape the shell's summon, hide and popout switching expect.
    property bool opened: false
    property bool popoutSwitchClosing: false
    function open() {
        popoutSwitchClosing = false;
        opened = true;
        // The vault is only swept every few minutes; a glance should never
        // show yesterday's numbers.
        Totals.refresh();
    }
    function close() {
        opened = false;
    }
    function closeForPopoutSwitch() {
        popoutSwitchClosing = true;
        close();
    }

    // The day's run, once there is one, is the part that moves; the rest of
    // the bar is a number that grows by a mile an hour at best.
    readonly property real todayNm: Totals.num(Totals.today && Totals.today.distanceNm)
    readonly property bool sailing: root.todayNm > 0

    readonly property string label: {
        if (!Totals.read) return "LOG";
        if (Totals.error !== "") return "LOG ?";
        if (!Totals.anything) return "LOG –";
        const run = Totals.distanceNm.toFixed(Totals.distanceNm >= 1000 ? 0 : 1) + " nm";
        return root.sailing ? "LOG " + run + " · +" + root.todayNm.toFixed(1) : "LOG " + run;
    }

    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight

    WidgetButton {
        id: button
        anchors.fill: parent
        bar: root.bar
        text: root.label
        foreground: root.bar ? root.bar.barForeground : Color.foreground
        dimmed: !Totals.anything || Totals.error !== ""
        tooltipText: {
            if (Totals.error !== "") return Totals.error;
            if (!Totals.anything) return "Nothing logged yet";
            if (root.sailing) return "Today: " + root.todayNm.toFixed(1) + " nm";
            return "";
        }
        onPressed: b => {
            if (b === Qt.LeftButton) {
                if (root.opened) root.close();
                else root.open();
            }
        }
    }

    KeyboardPanel {
        id: popup
        anchorItem: button
        bar: root.bar
        owner: root
        open: root.opened
        padding: 12
        borderSpec: Border.flat(Color.accent, 2)
        // Fixed size, as the other Omahoy widgets do: binding to the loaded
        // content makes the popover jump as it settles. Through
        // `Style.space` so it still follows the shell's own scale.
        contentWidth: Style.space(320)
        contentHeight: Style.space(290)
        focusTarget: content.item
        Loader {
            id: content
            anchors.fill: parent
            active: root.opened
            sourceComponent: Ledger {
                onCloseRequested: root.close()
            }
        }
    }
}
