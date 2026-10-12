#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

cannot_run=3
all_steps=(lint test comments qml engine goldens clean)
steps=("$@")
((${#steps[@]})) || steps=("${all_steps[@]}")

skip=()
if [[ -n ${VERIFY_SKIP:-} ]]; then
  IFS=',' read -r -a skip <<< "$VERIFY_SKIP"
fi

for s in "${steps[@]}" "${skip[@]}"; do
  [[ " ${all_steps[*]} " == *" $s "* ]] || {
    printf 'Unknown step: %s\n' "$s" >&2
    exit 2
  }
done

say() { printf '\n== %s\n' "$*"; }
skipped() {
  local s
  for s in "${skip[@]}"; do [[ $s == "$1" ]] && return 0; done
  return 1
}

before=$(git status --porcelain --untracked-files=all)

step_lint() { mise lint; }
step_test() { mise test; }

step_comments() {
  if [[ ! -f scripts/check-comments.sh ]]; then
    printf 'scripts/check-comments.sh is missing\n' >&2
    return 1
  fi
  printf '%s\n' 'f3ec8a2d07e8f1e7c50770d0bb93579fc7bb0d60fc8e359a22607ba55cebc6c6  scripts/check-comments.sh' | sha256sum -c || return 1
  scripts/check-comments.sh
}

step_qml() {
  local q
  compgen -G 'ui/*.qml' >/dev/null || {
    echo 'No ui/*.qml.'
    return "$cannot_run"
  }
  if [[ -n ${QMLLINT:-} ]]; then
    if [[ -f $QMLLINT && -x $QMLLINT ]]; then
      q=$QMLLINT
    elif q=$(command -v -- "$QMLLINT") && [[ -f $q && -x $q ]]; then
      :
    else
      printf 'QMLLINT=%s is not a program\n' "$QMLLINT" >&2
      return 1
    fi
  else
    q=$(command -v qmllint || command -v qmllint6 || command -v pyside6-qmllint || true)
    if [[ -z ${q:-} || ! -f $q || ! -x $q ]]; then
      echo 'qmllint is not installed.'
      return "$cannot_run"
    fi
  fi
  # Quickshell's modules are not installed for qmllint, so import, type,
  # property, and unqualified findings are about those modules.
  "$q" --max-warnings 0 --import disable --unresolved-type disable \
    --missing-property disable --unqualified disable ui/*.qml
}

step_engine() {
  if ! pkg-config --exists Qt6Core Qt6Gui Qt6Qml Qt6Network; then
    echo 'Qt6 QML dev packages not installed.'
    return "$cannot_run"
  fi
  local work rc
  work=$(mktemp -d)
  (
    set -euo pipefail
    export QT_QPA_PLATFORM=offscreen
    export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/tmp/runtime-omalogbook-verify}
    mkdir -p "$XDG_RUNTIME_DIR"
    chmod 700 "$XDG_RUNTIME_DIR"
    export PATH=/usr/bin:/bin

    g++ -fPIC -std=c++17 scripts/qs-url-probe.cpp -o "$work/qs-url-probe" \
      $(pkg-config --cflags --libs Qt6Core Qt6Gui Qt6Qml Qt6Network)

    failed=0

    parent_of() { printf '%s\n' "${1%/*}"; }

    mode_of() {
      local qml=$1 repo
      if grep -q 'function parentOf(' "$qml" \
        && grep -q 'readonly property string uiDir:' "$qml" \
        && grep -q 'readonly property string repo:' "$qml"; then
        printf '%s\n' resolved
        return
      fi
      repo=$(sed -n 's/^[[:space:]]*readonly property string repo:[[:space:]]*//p' "$qml")
      if [[ -n $repo ]] && [[ $(printf '%s\n' "$repo" | wc -l) -eq 1 ]]; then
        printf '%s\n' repo-line
        return
      fi
      if grep -q 'Quickshell.shellDir' "$qml"; then
        printf '%s\n' shelldir
        return
      fi
      printf '%s\n' unknown
    }

    start_script_of() {
      awk -F\' '/\[ -x "\$b" \] \|\| b=omalogbook/ { print $2; exit }' "$1"
    }

    write_probe() {
      local qml=$1 mode=$2 dest=$3
      local repo_expr ui_dir parent_fn
      repo_expr=$(sed -n 's/^[[:space:]]*readonly property string repo:[[:space:]]*//p' "$qml")
      if [[ $mode == resolved ]]; then
        ui_dir=$(sed -n 's/^[[:space:]]*readonly property string uiDir:[[:space:]]*//p' "$qml")
        parent_fn=$(awk '
          /^[[:space:]]*function parentOf\(/ { p = 1 }
          p { print }
          p && /^[[:space:]]*}[[:space:]]*$/ { exit }
        ' "$qml")
        [[ -n $ui_dir && -n $repo_expr && -n $parent_fn ]]
        cat > "$dest" <<EOF
import QtQuick
QtObject {
${parent_fn}
    readonly property string uiDir: ${ui_dir}
    readonly property string repo: ${repo_expr}
    Component.onCompleted: {
        console.log("REPO=" + repo)
        Qt.quit()
    }
}
EOF
      else
        [[ -n $repo_expr ]]
        cat > "$dest" <<EOF
import QtQuick
QtObject {
    readonly property string repo: ${repo_expr}
    Component.onCompleted: {
        console.log("REPO=" + repo)
        Qt.quit()
    }
}
EOF
      fi
    }

    check() {
      local name=$1 mode=$2 script=$3 file=$4 expect=$5 root=${6:-}
      local config_root out got binary ran ran_real want log
      if [[ -n $root ]]; then
        config_root=$(readlink -f "$root")
      else
        config_root=$(readlink -f "$(dirname "$file")")
      fi
      if [[ $mode == shelldir ]]; then
        got=$(parent_of "$config_root")
        binary="$config_root/../target/release/omalogbook"
        out="shellDir=$config_root"
      else
        if [[ -n $root ]]; then
          out=$("$work/qs-url-probe" "$file" "$root" 2>&1) || true
        else
          out=$("$work/qs-url-probe" "$file" 2>&1) || true
        fi
        got=$(printf '%s\n' "$out" | sed -n 's/^qml: REPO=//p' | tail -n 1)
        binary="$got/target/release/omalogbook"
      fi
      rm -rf "$work/rt"
      mkdir -p "$work/rt"
      log=$work/rt/engine.log
      : > "$log"
      rm -f "$work/ran"
      RECORD=$work/ran sh -c "$script" omalogbook-start "$binary" 2>>"$log" || true
      ran=$(cat "$work/ran" 2>/dev/null || true)
      if [[ -n $ran ]]; then
        ran_real=$(readlink -f "$ran")
      else
        ran_real=
      fi
      want=$(readlink -f "$expect/target/release/omalogbook")
      printf '\n== %s\n' "$name"
      printf '%s\n' "$out" | sed -n 's/^/  /p'
      printf '  repo: %s\n  want: %s\n  started: %s\n' "$got" "$expect" "${ran_real:-<none>}"
      if [[ -s $log ]]; then
        printf '  log: %s\n' "$(tail -n 1 "$log")"
      fi
      if [[ $got != "$expect" || $ran_real != "$want" ]]; then
        echo '  FAIL' >&2
        failed=1
      else
        echo '  ok'
      fi
    }

    run_file() {
      local qml=$1 label=$2 mode script
      mode=$(mode_of "$qml")
      script=$(start_script_of "$qml")
      if [[ -z $script || $mode == unknown ]]; then
        echo "FAIL: $qml does not name a repo and a PATH fallback" >&2
        failed=1
        return
      fi
      if ! grep -q 'OMALOGBOOK_BIN' "$qml"; then
        echo "FAIL: $qml dropped OMALOGBOOK_BIN" >&2
        failed=1
      fi
      if [[ $mode != shelldir ]]; then
        write_probe "$qml" "$mode" "$work/probe.qml"
        cp "$work/probe.qml" "$work/checkout/ui/probe.qml"
        cp "$work/probe.qml" "$work/shell/plugins/org.omahoy.logbook/ui/probe.qml"
      fi
      check "$label checkout, quickshell -p ui/shell.qml" \
        "$mode" "$script" \
        "$work/checkout/ui/probe.qml" "$work/checkout"
      check "$label plugin under the shell config root" \
        "$mode" "$script" \
        "$work/shell/plugins/org.omahoy.logbook/ui/probe.qml" \
        "$work/shell/plugins/org.omahoy.logbook" \
        "$work/shell"
    }

    mkdir -p "$work/checkout/ui" "$work/checkout/target/release" \
      "$work/shell/plugins/org.omahoy.logbook/ui" \
      "$work/shell/plugins/org.omahoy.logbook/target/release"
    cat > "$work/checkout/target/release/omalogbook" <<'EOF'
#!/bin/sh
printf '%s\n' "$0" > "$RECORD"
EOF
    chmod +x "$work/checkout/target/release/omalogbook"
    cp "$work/checkout/target/release/omalogbook" \
      "$work/shell/plugins/org.omahoy.logbook/target/release/omalogbook"

    run_file ui/Book.qml Book
    run_file ui/Totals.qml Totals

    if ! grep -q 'wrapMode: Text.Wrap' ui/LogbookWindow.qml \
      || ! grep -q 'engineStatus' ui/LogbookWindow.qml \
      || ! grep -q 'book.lastLog' ui/LogbookWindow.qml; then
      echo 'FAIL: the window does not wrap book.lastLog in the body' >&2
      failed=1
    else
      echo 'window surfaces book.lastLog'
    fi

    if ((failed)); then
      echo 'FAILED' >&2
      exit 1
    fi
    echo 'All engine-path checks passed.'
  )
  rc=$?
  rm -rf "$work"
  return "$rc"
}

step_goldens() { mise goldens; }

step_clean() {
  local after
  after=$(git status --porcelain --untracked-files=all)
  if [[ $after != "$before" ]]; then
    printf 'Verification changed the tree. Tests must write to a temp dir:\n'
    diff <(printf '%s\n' "$before") <(printf '%s\n' "$after") | sed -n 's/^> /  /p'
    return 1
  fi
}

passed=()
not_run=()
failed=()
for s in "${steps[@]}"; do
  if skipped "$s"; then
    not_run+=("$s")
    continue
  fi
  say "$s"
  rc=0
  "step_$s" || rc=$?
  case $rc in
    0) passed+=("$s") ;;
    "$cannot_run") not_run+=("$s") ;;
    *) failed+=("$s") ;;
  esac
done

summary=$(printf '\nPassed: %s' "${passed[*]:-nothing}")
if ((${#not_run[@]})); then
  summary+=$(printf '\nNot run: %s' "${not_run[*]}")
fi
if ((${#failed[@]})); then
  printf '%s\nFAILED: %s\n' "$summary" "${failed[*]}" >&2
  exit 1
fi
printf '%s\n' "$summary"
