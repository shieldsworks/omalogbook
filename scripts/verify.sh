#!/usr/bin/env bash
# The one command that says a change to omalogbook is done.
# Agents run it before calling a task finished. CI runs it with
# VERIFY_SKIP=qml, and runs the qml step in its own job with QMLLINT set.
#
#   scripts/verify.sh
#   scripts/verify.sh goldens qml
#   VERIFY_SKIP=lint,test scripts/verify.sh
#   QMLLINT=/path/to/qmllint scripts/verify.sh qml
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

all_steps=(lint test comments qml goldens clean)
steps=("$@")
((${#steps[@]})) || steps=("${all_steps[@]}")

skip=()
if [[ -n ${VERIFY_SKIP:-} ]]; then
  IFS=',' read -r -a skip <<< "$VERIFY_SKIP"
fi

# A name that is not a step would otherwise skip nothing, or run nothing.
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

# Snapshot the tree first, so "clean" can tell files this run created
# from work that was already there.
before=$(git status --porcelain --untracked-files=all)

# A step returns 0 when it passed, 3 when it could not run, and
# anything else when it failed.
step_lint() { mise lint; }
step_test() { mise test; }

step_comments() {
  if [[ ! -f scripts/check-comments.sh ]]; then
    printf 'scripts/check-comments.sh is missing\n' >&2
    return 1
  fi
  scripts/check-comments.sh
}

step_qml() {
  local q
  compgen -G 'ui/*.qml' >/dev/null || {
    echo 'No ui/*.qml.'
    return 3
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
      return 3
    fi
  fi
  # Quickshell's modules are not installed for qmllint, so import, type,
  # property, and unqualified findings are about those modules.
  "$q" --max-warnings 0 --import disable --unresolved-type disable \
    --missing-property disable --unqualified disable ui/*.qml
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
    3) not_run+=("$s") ;;
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
