#!/usr/bin/env bash
if [[ -n ${BASH_ENV:-} ]]; then
  if ! unset BASH_ENV; then
    exit 1
  fi
  exec env -u BASH_ENV bash --noprofile --norc "$0" "$@"
fi

set -euo pipefail
set -m

repo=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$repo"

run=
evidence=
bin=
real_home=${HOME:-}
self_pgid=$(ps -o pgid= -p $$ | tr -d ' ')
all_pgids=
started_pid=
follow_pid=
follow_pgid=
hub_pid=
hub_pgid=
window_pid=
window_pgid=
grim_pid=
grim_pgid=
pause_pid=
pause_pgid=
inherit_pgid=
copy_failed=0
outside_failed=0
kept_run=
proved=0
launch_n=0
spawned_pids=
SOCKET=

if [[ -n ${EVIDENCE_DIR:-} ]]; then
  mkdir -p "$EVIDENCE_DIR"
  evidence=$(mktemp -d "$EVIDENCE_DIR/drive.XXXXXX")
else
  evidence=$(mktemp -d /tmp/omalogbook-verify-evidence.XXXXXX)
fi

# Parent exports are not the window's binary or its size. The window
# command sets OMALOGBOOK_BIN itself.
for seam in OMALOGBOOK_BIN OMALOGBOOK_THEME_DIR OMALOGBOOK_WIDTH OMALOGBOOK_HEIGHT; do
  if [[ -n ${!seam:-} ]]; then
    printf 'ignored %s\n' "$seam" >>"$evidence/seams.txt"
    unset "$seam"
  fi
done

add_pgid() {
  local pgid=${1:-}
  [[ -n $pgid && $pgid != "$self_pgid" ]] || return 0
  case " $all_pgids " in
    *" $pgid "*) ;;
    *) all_pgids="$all_pgids $pgid" ;;
  esac
}

note_spawn() {
  local pid=${1:-}
  [[ -n $pid && $pid != "$self_pgid" ]] || return 0
  case " $spawned_pids " in
    *" $pid "*) ;;
    *) spawned_pids="$spawned_pids $pid" ;;
  esac
  add_pgid "$pid"
}

stop_spawned() {
  local pid
  for pid in $spawned_pids; do
    [[ -n $pid && $pid != "$self_pgid" ]] || continue
    kill -TERM -- "-$pid" 2>/dev/null || true
    kill -TERM "$pid" 2>/dev/null || true
  done
  for pid in $spawned_pids; do
    [[ -n $pid && $pid != "$self_pgid" ]] || continue
    kill -KILL -- "-$pid" 2>/dev/null || true
    kill -KILL "$pid" 2>/dev/null || true
  done
}

note_tree() {
  local pid=${1:-} child pgid
  [[ -n $pid ]] || return 0
  pgid=$(ps -o pgid= -p "$pid" 2>/dev/null | tr -d ' ' || true)
  add_pgid "$pgid"
  kill -0 "$pid" 2>/dev/null || return 0
  for child in $(ps -o pid= --ppid "$pid" 2>/dev/null || true); do
    note_tree "$child"
  done
}

start_session() {
  local out=$1 err=$2
  shift 2
  set +m
  if [[ $out == "$err" ]]; then
    setsid "$@" >"$out" 2>&1 </dev/null &
  else
    setsid "$@" >"$out" 2>"$err" </dev/null &
  fi
  started_pid=$!
  disown "$started_pid" 2>/dev/null || true
  set -m
  note_spawn "$started_pid"
}

signal_pgids() {
  local sig=$1 pgid
  shift
  for pgid in "$@"; do
    [[ -n $pgid && $pgid != "$self_pgid" ]] || continue
    kill "-$sig" -- "-$pgid" 2>/dev/null || true
  done
}

sweep_run() {
  local pid cwd cmd envfile
  [[ -n ${run:-} && -d $run ]] || return 0
  for pid in /proc/[0-9]*; do
    pid=${pid#/proc/}
    [[ $pid == "$$" ]] && continue
    cwd=$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)
    cmd=$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true)
    envfile=/proc/$pid/environ
    if [[ $cwd == "$run" || $cwd == "$run"/* || $cmd == *"$run"* ]]; then
      kill -KILL "$pid" 2>/dev/null || true
      continue
    fi
    if [[ -r $envfile ]] && grep -z -F -q -- "XDG_RUNTIME_DIR=$run" "$envfile"; then
      kill -KILL "$pid" 2>/dev/null || true
    fi
  done
}

stop_groups() {
  local pgid pid before have
  local -a ids=()
  before=" $all_pgids "
  for pgid in "$@"; do
    [[ -n $pgid && $pgid != "$self_pgid" ]] || continue
    note_tree "$pgid"
    ids+=("$pgid")
  done
  have=" ${ids[*]} "
  for pgid in $all_pgids; do
    case $before in
      *" $pgid "*) continue ;;
    esac
    case $have in
      *" $pgid "*) continue ;;
    esac
    ids+=("$pgid")
    have="$have$pgid "
  done
  ((${#ids[@]})) || return 0
  signal_pgids TERM "${ids[@]}"
  signal_pgids KILL "${ids[@]}"
  for pgid in "${ids[@]}"; do
    for pid in $(pgrep -g "$pgid" 2>/dev/null || true); do
      kill -KILL "$pid" 2>/dev/null || true
    done
  done
}

stop_all() {
  local pgid pid
  local -a ids=()
  stop_spawned
  note_tree "${follow_pid:-}"
  note_tree "${hub_pid:-}"
  note_tree "${window_pid:-}"
  note_tree "${grim_pid:-}"
  note_tree "${pause_pid:-}"
  note_tree "${inherit_pgid:-}"
  for pgid in $all_pgids; do
    ids+=("$pgid")
  done
  signal_pgids TERM "${ids[@]}"
  signal_pgids KILL "${ids[@]}"
  for pgid in "${ids[@]}"; do
    for pid in $(pgrep -g "$pgid" 2>/dev/null || true); do
      kill -KILL "$pid" 2>/dev/null || true
    done
  done
  sweep_run
  follow_pid=
  follow_pgid=
  hub_pid=
  hub_pgid=
  window_pid=
  window_pgid=
  grim_pid=
  grim_pgid=
  pause_pid=
  pause_pgid=
  inherit_pgid=
  all_pgids=
}

collect_watch() {
  local dest=$1 raw
  raw=$(mktemp "$run/watch.XXXXXX")
  # GNU find exits 1 when a directory is not readable. pipefail would
  # abort the drive before it prints run:. The snapshot keeps every
  # file find could read.
  {
    find "$(dirname "$run")" \
      \( -path "$run" -o -path "$run/*" -o -path "$evidence" -o -path "$evidence/*" \
         -o -path "$repo/.git" -o -path "$repo/.git/*" \) -prune \
      -o -type f -printf '%p %T@ %s\n' 2>/dev/null || true
    if [[ -n ${real_home:-} && -d $real_home ]]; then
      find "$real_home" \
        \( -path "$real_home/.cargo" -o -path "$real_home/.cargo/*" \
           -o -path "$real_home/.rustup" -o -path "$real_home/.rustup/*" \
           -o -path "$real_home/.cache" -o -path "$real_home/.cache/*" \
           -o -path "$real_home/.npm" -o -path "$real_home/.npm/*" \
           -o -path "$real_home/.cursor" -o -path "$real_home/.cursor/*" \
           -o -path "$real_home/.cursor-server" -o -path "$real_home/.cursor-server/*" \
           -o -path "$real_home/.local/share/mise" -o -path "$real_home/.local/share/mise/*" \
           -o -path "$real_home/.local/share/cursor" -o -path "$real_home/.local/share/cursor/*" \
           -o -path "$repo/.git" -o -path "$repo/.git/*" \
        \) -prune \
        -o -type f -printf '%p %T@ %s\n' 2>/dev/null || true
    fi
  } | LC_ALL=C sort -u >"$raw"
  mv "$raw" "$dest"
}

confirm_drive() {
  local git_diff=0 outside_diff=0
  [[ -n ${run:-} && -d $run && -f $run/watch-before && -f $run/git-before ]] || return 1
  git -C "$repo" status --porcelain --untracked-files=all >"$run/git-after"
  cmp -s "$run/git-before" "$run/git-after" || git_diff=1
  collect_watch "$run/watch-after"
  cmp -s "$run/watch-before" "$run/watch-after" || outside_diff=1
  if [[ $git_diff -ne 0 || $outside_diff -ne 0 ]]; then
    {
      printf 'git_diff=%s outside_diff=%s\n' "$git_diff" "$outside_diff"
      diff -u "$run/git-before" "$run/git-after" || true
      diff -u "$run/watch-before" "$run/watch-after" || true
    } >"$run/outside-diff.txt"
    return 1
  fi
}

archive_current() {
  local dest marker ok old diff_ok
  [[ -n ${run:-} && -d ${run:-} ]] || return 0
  stop_all
  launch_n=$((launch_n + 1))
  dest=$evidence/launch-$launch_n
  marker=$run/artifacts/.copy-marker
  ok=0
  diff_ok=1
  mkdir -p "$run/artifacts" "$dest"
  printf '%s\n' "$run" >"$marker"
  if cp -a "$run/artifacts/." "$dest/"; then
    if [[ -f $dest/.copy-marker && $(cat "$dest/.copy-marker") == "$run" ]]; then
      ok=1
    fi
  fi
  if [[ -f $run/outside-diff.txt ]]; then
    cp -a "$run/outside-diff.txt" "$dest/outside-diff.txt" || diff_ok=0
  elif [[ $outside_failed -eq 1 ]]; then
    diff_ok=0
  fi
  if [[ $ok -eq 1 && $diff_ok -eq 1 ]]; then
    proved=1
    old=$run
    run=
    rm -rf "$old"
  else
    copy_failed=1
    kept_run=$run
    printf 'copy failed, run dir kept: %s\n' "$run" >&2
    run=
  fi
}

finish() {
  # An INT during a function makes bash drop that function context before
  # the EXIT trap runs. `local` then fails and `set -e` aborts cleanup.
  status=$?
  set +e
  trap '' INT TERM
  trap - EXIT
  stop_spawned
  if [[ -n ${run:-} && -d $run && -f ${run:-}/watch-before ]]; then
    if ! confirm_drive; then
      outside_failed=1
      printf 'wrote outside %s\n' "$run" >&2
      if [[ $status -eq 0 ]]; then
        status=1
      fi
    fi
  fi
  archive_current
  if [[ $copy_failed -eq 1 || $proved -ne 1 ]]; then
    if [[ $status -eq 0 ]]; then
      status=1
    fi
    printf 'evidence: none\n'
  else
    printf 'evidence: %s\n' "$evidence"
  fi
  exit "$status"
}
trap finish EXIT
trap 'trap "" INT TERM; exit 130' INT
trap 'trap "" INT TERM; exit 143' TERM

canon_path() {
  realpath -m -- "$1"
}

inside_run() {
  local canon run_canon
  [[ -n ${run:-} ]] || return 1
  [[ -n ${1:-} ]] || return 1
  canon=$(canon_path "$1") || return 1
  run_canon=$(canon_path "$run") || return 1
  [[ $canon == "$run_canon"/* ]]
}

write_config() {
  local vault=$1
  mkdir -p "$run/config/omalogbook"
  printf 'vault = "%s"\nboat = "Dash"\ngit = false\n' "$vault" >"$run/config/omalogbook/config.toml"
}

config_vault_raw() {
  local file=$run/config/omalogbook/config.toml line
  [[ -f $file ]] || return 1
  line=$(sed -n 's/^vault = "\([^"]*\)"/\1/p' "$file" | head -1)
  [[ -n $line ]] || return 1
  printf '%s\n' "$line"
}

require_vault() {
  local raw
  raw=$(config_vault_raw) || return 1
  inside_run "$raw"
}

require_env_dirs() {
  inside_run "$run/home" || return 1
  inside_run "$run/config" || return 1
  inside_run "$run/runtime" || return 1
  inside_run "$run/vault" || return 1
}

require_cli_vaults() {
  local prev="" arg
  for arg in "$@"; do
    if [[ $prev == --vault ]]; then
      inside_run "$arg" || return 1
    fi
    prev=$arg
  done
}

launch_bin() {
  require_env_dirs || return 1
  require_vault || return 1
  require_cli_vaults "$@" || return 1
  env \
    -u OMALOGBOOK_BIN \
    -u OMALOGBOOK_THEME_DIR \
    -u OMALOGBOOK_WIDTH \
    -u OMALOGBOOK_HEIGHT \
    HOME="$run/home" \
    XDG_CONFIG_HOME="$run/config" \
    XDG_RUNTIME_DIR="$run/runtime" \
    TZ=UTC \
    "$bin" "$@"
}

record() {
  local id=$1 dir code
  shift
  dir=$run/artifacts/verify/$id
  mkdir -p "$dir"
  printf '%q ' "$@" >"$dir/command.txt"
  printf '\n' >>"$dir/command.txt"
  set +e
  "$@" >"$dir/stdout.txt" 2>"$dir/stderr.txt"
  code=$?
  set -e
  printf '%s\n' "$code" >"$dir/exit-code.txt"
  [[ -s $dir/stdout.txt ]] || rm -f "$dir/stdout.txt"
  [[ -s $dir/stderr.txt ]] || rm -f "$dir/stderr.txt"
  return 0
}

exit_is() {
  local id=$1 want=$2 got
  got=$(cat "$run/artifacts/verify/$id/exit-code.txt")
  [[ $got -eq $want ]]
}

stdout_has() {
  local id=$1
  [[ -f $run/artifacts/verify/$id/stdout.txt ]] || return 1
  grep -F -q -- "$2" "$run/artifacts/verify/$id/stdout.txt"
}

stderr_has() {
  local id=$1
  [[ -f $run/artifacts/verify/$id/stderr.txt ]] || return 1
  grep -F -q -- "$2" "$run/artifacts/verify/$id/stderr.txt"
}

stderr_empty() {
  [[ ! -e $run/artifacts/verify/$1/stderr.txt ]]
}

boat_day() {
  TZ=UTC date +%F
}

note_path() {
  local day
  day=$(boat_day)
  printf '%s\n' "$run/vault/${day:0:4}/${day:5:2}/$day.md"
}

reset_vault() {
  rm -rf "$run/vault"
  mkdir -p "$run/vault"
  write_config "$run/vault"
}

art_dir() {
  mkdir -p "$run/artifacts/verify/$1"
  printf '%s\n' "$run/artifacts/verify/$1"
}

prepare_tree() {
  run=$(mktemp -d /tmp/omalogbook-verify.XXXXXX)
  [[ $run == /* ]] || exit 1
  mkdir -p "$run/home" "$run/config/omalogbook" "$run/runtime" "$run/vault" "$run/artifacts/verify"
  write_config "$run/vault"
  : >"$run/stamp"
  git -C "$repo" status --porcelain --untracked-files=all >"$run/git-before"
  collect_watch "$run/watch-before"
}

prepare() {
  export PATH="${HOME}/.local/bin:${PATH:-}"
  mise install
  mise exec -- cargo build --locked
  prepare_tree
  bin=$(realpath ./target/debug/omalogbook)
  [[ -x $bin ]] || exit 1
}

prepare_bare() {
  prepare_tree
  bin=$run/sentinel
}

pid_on_socket() {
  local pid=${1:-} path=${2:-${SOCKET:-}}
  [[ -n $pid && -n $path ]] || return 1
  python3 - "$pid" "$path" <<'PY'
import os
import sys

pid, path = sys.argv[1], sys.argv[2]
listening = set()
with open("/proc/net/unix") as fh:
    next(fh)
    for line in fh:
        parts = line.split()
        if len(parts) >= 8 and parts[5] == "01" and parts[7] == path:
            listening.add(parts[6])
if not listening:
    raise SystemExit(1)
held = False
try:
    names = os.listdir(f"/proc/{pid}/fd")
except OSError:
    raise SystemExit(1)
for name in names:
    try:
        target = os.readlink(f"/proc/{pid}/fd/{name}")
    except OSError:
        continue
    if target.startswith("socket:[") and target.endswith("]"):
        inode = target[len("socket:[") : -1]
        if inode in listening:
            held = True
            break
if not held:
    raise SystemExit(1)
try:
    import subprocess
    env = dict(os.environ)
    env["COLUMNS"] = "512"
    out = subprocess.check_output(["ss", "-lxp"], text=True, stderr=subprocess.DEVNULL, env=env)
except (OSError, subprocess.CalledProcessError):
    raise SystemExit(1)
ok = False
for line in out.splitlines():
    if "LISTEN" not in line:
        continue
    if path not in line.split():
        continue
    if f"pid={pid}," in line or f"pid={pid})" in line:
        ok = True
        break
raise SystemExit(0 if ok else 1)
PY
}

listener_pid() {
  local path=$1
  python3 - "$path" <<'PY'
import os
import subprocess
import sys

path = sys.argv[1]
env = dict(os.environ)
env["COLUMNS"] = "512"
out = subprocess.check_output(["ss", "-lxp"], text=True, stderr=subprocess.DEVNULL, env=env)
for line in out.splitlines():
    if "LISTEN" not in line:
        continue
    if path not in line.split():
        continue
    flat = line.replace("(", " ").replace(")", " ").replace(",", " ")
    for token in flat.split():
        if token.startswith("pid="):
            print(token.split("=", 1)[1])
            raise SystemExit(0)
raise SystemExit(1)
PY
}

protocol_line() {
  local path=$1
  python3 - "$path" <<'PY'
import json
import socket
import sys

path = sys.argv[1]
s = socket.socket(socket.AF_UNIX)
s.settimeout(0.4)
try:
    s.connect(path)
    data = b""
    while b"\n" not in data and len(data) < 65536:
        chunk = s.recv(4096)
        if not chunk:
            break
        data += chunk
except OSError:
    raise SystemExit(1)
line = data.split(b"\n", 1)[0].decode("utf-8", "replace")
try:
    msg = json.loads(line)
except json.JSONDecodeError:
    raise SystemExit(1)
if not isinstance(msg, dict):
    raise SystemExit(1)
if msg.get("type") not in ("hello", "state", "targets"):
    raise SystemExit(1)
if "v" not in msg:
    raise SystemExit(1)
sys.stdout.write(line + "\n")
PY
}

run_ready() {
  local art=$1 i listener note stdout
  [[ -n ${SOCKET:-} && -S $SOCKET ]] || return 1
  [[ -n ${follow_pid:-} ]] || return 1
  note=$(note_path)
  stdout=$art/stdout.txt
  for i in $(seq 1 12); do
    listener=$(listener_pid "$SOCKET" || true)
    if [[ -n $listener ]] \
      && pid_on_socket "$listener" "$SOCKET" \
      && protocol_line "$SOCKET" >/dev/null \
      && [[ -f $stdout ]] \
      && grep -F -q -- "omalogbook:" "$stdout" \
      && grep -F -q -- "$note" "$stdout"; then
      printf 'ready:yes\nlistener:%s\n' "$listener" | tee "$art/ready.txt"
      return 0
    fi
    sleep 0.25
  done
  printf 'ready:no\n' | tee "$art/ready.txt"
  return 1
}

write_hub() {
  local kind=$1
  cat >"$run/$kind.py" <<'PY'
import socket
import sys
import threading

kind, path = sys.argv[1], sys.argv[2]
state = (
    '{"type":"state","v":1,"fix":{"status":"ok","lat":37.864711,'
    '"lon":-122.3207314,"sogKn":5.0,"cogDeg":255.0,'
    '"utc":"2026-09-13T21:00:10Z","satellites":9,"hdop":0.9,"ageSeconds":0}}\n'
)

def hold(conn):
    try:
        if kind == "protocol":
            conn.sendall(state.encode())
        conn.settimeout(30)
        conn.recv(1)
    except OSError:
        pass
    finally:
        conn.close()

srv = socket.socket(socket.AF_UNIX)
srv.bind(path)
srv.listen(8)
while True:
    conn, _ = srv.accept()
    threading.Thread(target=hold, args=(conn,), daemon=True).start()
PY
}

drive_doctor() {
  local printed
  require_env_dirs
  require_vault
  record doctor launch_bin --version
  exit_is doctor 0
  stdout_has doctor "omalogbook "
  stderr_empty doctor
  cat "$run/artifacts/verify/doctor/stdout.txt"
  record vault-path launch_bin vault
  exit_is vault-path 0
  stderr_empty vault-path
  printed=$(cat "$run/artifacts/verify/vault-path/stdout.txt")
  inside_run "$printed"
  [[ $printed == "$run/vault" ]]
  printf '%s\n' "$printed"
}

drive_note() {
  local art day path
  art=$(art_dir note)
  reset_vault
  day=$(boat_day)
  record note-words launch_bin note "Dolphins off the port side"
  exit_is note-words 0
  stderr_empty note-words
  path=$(note_path)
  [[ $(cat "$run/artifacts/verify/note-words/stdout.txt") == "$path" ]]
  grep -F "Dolphins off the port side" "$path" >/dev/null
  grep -F "<!-- omalogbook:begin -->" "$path" >/dev/null
  grep -F "°" "$path" >/dev/null && return 1 || true
  record note-today launch_bin today
  exit_is note-today 0
  stdout_has note-today "$day · Dash"
  stdout_has note-today "Dolphins off the port side"
  stdout_has note-today "0.0 nm · fastest 0.0 kn · under way 0 min"
  record note-mark launch_bin note /anchor "25 ft, 5:1"
  exit_is note-mark 0
  grep -F "Anchor down: 25 ft, 5:1" "$path" >/dev/null
  grep -F "wind" "$path" >/dev/null && return 1 || true
  grep -F "HRRR" "$path" >/dev/null && return 1 || true
  record note-unknown launch_bin note /madeup
  exit_is note-unknown 0
  stderr_has note-unknown "/madeup isn't a mark"
  grep -F "/madeup" "$path" >/dev/null
  cp -a "$path" "$art/before-empty.md"
  record note-empty launch_bin note
  exit_is note-empty 64
  stderr_has note-empty "what should the entry say?"
  cmp -s "$art/before-empty.md" "$path"
  record note-flag launch_bin note --vault "$run/not-the-vault" "flag check"
  exit_is note-flag 0
  [[ ! -e $run/not-the-vault ]]
  grep -F -- "--vault" "$path" >/dev/null
  grep -F "flag check" "$path" >/dev/null
  record note-presets launch_bin presets
  exit_is note-presets 0
  stdout_has note-presets "/depart  left the berth; notes the sunset and starts the trip"
  stdout_has note-presets "/berth"
  cp -a "$path" "$art/note.md"
}

drive_today() {
  local art path
  art=$(art_dir today)
  reset_vault
  cp -a tests/golden/morning/. "$run/vault/"
  record today-morning launch_bin today --date 2026-09-13
  exit_is today-morning 0
  stdout_has today-morning "2026-09-13 · Dash"
  stdout_has today-morning "log opened"
  stdout_has today-morning "under way"
  stdout_has today-morning "stopped"
  stdout_has today-morning "1.0 nm"
  stdout_has today-morning "under way 19 min"
  [[ -f $run/vault/tracks/2026-09-13-120100.gpx ]]
  record today-json launch_bin today --date 2026-09-13 --json
  exit_is today-json 0
  stdout_has today-json '"v":1'
  stdout_has today-json '"date":"2026-09-13"'
  stdout_has today-json '"boat":"Dash"'
  stdout_has today-json '"distanceNm":1.0'
  stdout_has today-json "under way"
  record today-path launch_bin path --date 2026-09-13
  exit_is today-path 0
  path=$run/vault/2026/09/2026-09-13.md
  [[ $(cat "$run/artifacts/verify/today-path/stdout.txt") == "$path" ]]
  record today-empty launch_bin today --date 2026-01-02
  exit_is today-empty 0
  [[ $(cat "$run/artifacts/verify/today-empty/stdout.txt") == "2026-01-02 · Dash" ]]
  [[ ! -e $run/vault/2026/01/2026-01-02.md ]]
  record today-empty-path launch_bin path --date 2026-01-02
  exit_is today-empty-path 0
  [[ $(cat "$run/artifacts/verify/today-empty-path/stdout.txt") == "$run/vault/2026/01/2026-01-02.md" ]]
  [[ ! -e $run/vault/2026/01/2026-01-02.md ]]
  record today-bad-date launch_bin today --date Tuesday
  exit_is today-bad-date 64
  stderr_has today-bad-date "--date wants a day like 2026-09-18"
  cp -a "$run/artifacts/verify/today-morning/stdout.txt" "$art/today.txt"
}

entry_text() {
  local n=$1 file=$2
  sed -n "s/^[[:space:]]*$n\\. //p" "$file" | head -1
}

drive_totals() {
  art_dir totals >/dev/null
  rm -rf "$run/vault" "$run/morning"
  mkdir -p "$run/vault" "$run/morning"
  write_config "$run/vault"
  cp -a tests/golden/morning/. "$run/morning/"
  record totals-empty launch_bin totals
  exit_is totals-empty 0
  [[ $(cat "$run/artifacts/verify/totals-empty/stdout.txt") == "Dash · nothing logged yet" ]]
  record totals-morning launch_bin totals --vault "$run/morning"
  exit_is totals-morning 0
  stdout_has totals-morning "Dash · 2026-09-13 to 2026-09-13 · 1 day sailed of 1"
  stdout_has totals-morning "1.0 nm"
  stdout_has totals-morning "19 min"
  stdout_has totals-morning "3.0 kn under way"
  stdout_has totals-morning "4.7 kn on 2026-09-13"
  record totals-json launch_bin totals --vault "$run/morning" --json
  exit_is totals-json 0
  stdout_has totals-json '"v":1'
  stdout_has totals-json '"boat":"Dash"'
  stdout_has totals-json '"distanceNm":1.0'
  stdout_has totals-json '"date":"2026-09-13"'
  record totals-again launch_bin totals
  exit_is totals-again 0
  [[ $(cat "$run/artifacts/verify/totals-again/stdout.txt") == "Dash · nothing logged yet" ]]
  [[ -z $(find "$run/vault" -type f -print) ]]
}

drive_correct() {
  local art line path today
  art=$(art_dir correct)
  reset_vault
  record correct-a launch_bin note "Dolphins off the port side"
  exit_is correct-a 0
  record correct-b launch_bin note "Crab boat crossed astern"
  exit_is correct-b 0
  record correct-today launch_bin today
  exit_is correct-today 0
  today=$run/artifacts/verify/correct-today/stdout.txt
  record correct-past launch_bin strike 9
  exit_is correct-past 1
  stderr_has correct-past "has 2 entries"
  grep -F "Dolphins off the port side" "$(note_path)" >/dev/null
  grep -F "Crab boat crossed astern" "$(note_path)" >/dev/null
  line=$(entry_text 1 "$today")
  [[ -n $line ]]
  record correct-amend launch_bin amend 1 --expect "$line" "Dolphins off the starboard side"
  exit_is correct-amend 0
  stdout_has correct-amend "Dolphins off the starboard side"
  stdout_has correct-amend "**"
  record correct-today2 launch_bin today
  exit_is correct-today2 0
  today=$run/artifacts/verify/correct-today2/stdout.txt
  stdout_has correct-today2 "Dolphins off the starboard side"
  stdout_has correct-today2 "Crab boat crossed astern"
  line=$(entry_text 1 "$today")
  record correct-strike launch_bin strike 1 --expect "$line"
  exit_is correct-strike 0
  stdout_has correct-strike "~~"
  stdout_has correct-strike "Dolphins off the starboard side"
  record correct-today3 launch_bin today
  exit_is correct-today3 0
  today=$run/artifacts/verify/correct-today3/stdout.txt
  [[ $(grep -cE '^[0-9]+\. ' "$today" || true) -eq 2 ]]
  line=$(entry_text 2 "$today")
  record correct-erase launch_bin strike 2 --expect "$line" --erase
  exit_is correct-erase 0
  path=$(note_path)
  [[ $(cat "$run/artifacts/verify/correct-erase/stdout.txt") == "$path" ]]
  record correct-today4 launch_bin today
  exit_is correct-today4 0
  if [[ -f $run/artifacts/verify/correct-today4/stdout.txt ]] \
    && grep -F -q "Crab boat crossed astern" "$run/artifacts/verify/correct-today4/stdout.txt"; then
    return 1
  fi
  stdout_has correct-today4 "Dolphins off the starboard side"
  record correct-stale launch_bin amend 1 --expect "not the line" "nope"
  exit_is correct-stale 1
  stderr_has correct-stale "that entry has changed since you read it; look again"
  record correct-today5 launch_bin today
  exit_is correct-today5 0
  if [[ -f $run/artifacts/verify/correct-today5/stdout.txt ]] \
    && grep -F -q "nope" "$run/artifacts/verify/correct-today5/stdout.txt"; then
    return 1
  fi
  cp -a "$path" "$art/note.md"
}

window_miss() {
  local id=$1 name=$2 dir
  dir=$run/artifacts/verify/$id
  mkdir -p "$dir"
  printf '%s\n' "$name" >"$dir/command.txt"
  printf 'missing command: %s\n' "$name" >"$dir/stdout.txt"
  printf '%s\n' '127' >"$dir/exit-code.txt"
  printf 'missing command: %s\n' "$name" >"$dir/miss.txt"
}

window_dead() {
  local dir=$run/artifacts/verify/window-painted
  mkdir -p "$dir"
  printf '%s\n' 'grim -' >"$dir/command.txt"
  printf '%s\n' 'process was not alive' >"$dir/stdout.txt"
  printf '%s\n' '1' >"$dir/exit-code.txt"
  printf '%s\n' 'process was not alive' >"$dir/miss.txt"
}

window_up() {
  [[ -n ${window_pid:-} && -n ${window_pgid:-} ]] || return 1
  kill -0 "$window_pid" 2>/dev/null || return 1
  kill -0 -- "-$window_pgid" 2>/dev/null
}

drive_window() {
  local art wayland runtime code dir painted_ok _
  local -a cmd
  art=$(art_dir window)
  reset_vault
  cp -a tests/golden/morning/. "$run/vault/"
  {
    printf 'DISPLAY=%s\n' "${DISPLAY:-unset}"
    printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY:-unset}"
    command -v quickshell || echo "quickshell missing"
    command -v Hyprland || echo "Hyprland missing"
    command -v grim || echo "grim missing"
    command -v qmllint || echo "qmllint missing"
    if pkg-config --exists Qt6Core Qt6Gui Qt6Qml Qt6Network; then
      echo "Qt6 present"
    else
      echo "Qt6 missing"
    fi
  } | tee "$art/tools.txt"
  wayland=${WAYLAND_DISPLAY-}
  if [[ -n $wayland && $wayland != /* ]]; then
    runtime=${XDG_RUNTIME_DIR-}
    [[ -n $runtime && $runtime == /* ]]
    wayland=$runtime/$wayland
  fi
  cmd=(env
    -u OMALOGBOOK_THEME_DIR
    -u OMALOGBOOK_WIDTH
    -u OMALOGBOOK_HEIGHT
    HOME="$run/home"
    XDG_CONFIG_HOME="$run/config"
    XDG_RUNTIME_DIR="$run/runtime"
    TZ=UTC
    OMALOGBOOK_BIN="$bin")
  if [[ -n $wayland ]]; then
    cmd+=(WAYLAND_DISPLAY="$wayland")
  fi
  cmd+=(./run.sh)
  painted_ok=1
  if ! command -v quickshell >/dev/null 2>&1; then
    record window-launch "${cmd[@]}"
    exit_is window-launch 127
    stderr_has window-launch "quickshell"
    window_miss window-painted quickshell
    painted_ok=0
  else
    dir=$run/artifacts/verify/window-launch
    mkdir -p "$dir"
    printf '%q ' "${cmd[@]}" >"$dir/command.txt"
    printf '\n' >>"$dir/command.txt"
    start_session "$dir/stdout.txt" "$dir/stderr.txt" "${cmd[@]}"
    window_pid=$started_pid
    window_pgid=$started_pid
    code=0
    for _ in $(seq 20); do
      if window_up; then
        break
      fi
      sleep 0.1
    done
    if ! window_up; then
      set +e
      wait "$window_pid" 2>/dev/null
      code=$?
      set -e
      printf '%s\n' "$code" >"$dir/exit-code.txt"
      window_dead
      stop_groups "$window_pgid"
      window_pid=
      window_pgid=
      painted_ok=0
    elif ! command -v grim >/dev/null 2>&1; then
      window_miss window-painted grim
      painted_ok=0
      stop_groups "$window_pgid"
      set +e
      wait "$window_pid" 2>/dev/null
      set -e
      window_pid=
      window_pgid=
    else
      dir=$run/artifacts/verify/window-painted
      mkdir -p "$dir"
      printf '%s\n' 'timeout -k 5 10 grim -' >"$dir/command.txt"
      set +m
      setsid timeout -k 5 10 grim - >"$dir/stdout.txt" 2>"$dir/stderr.txt" </dev/null &
      grim_pid=$!
      grim_pgid=$grim_pid
      set -m
      note_spawn "$grim_pid"
      set +e
      wait "$grim_pid"
      code=$?
      set -e
      grim_pid=
      printf '%s\n' "$code" >"$dir/exit-code.txt"
      [[ -s $dir/stdout.txt ]] || rm -f "$dir/stdout.txt"
      [[ -s $dir/stderr.txt ]] || rm -f "$dir/stderr.txt"
      if ! window_up; then
        painted_ok=0
        printf '%s\n' 'process was not alive' >"$dir/miss.txt"
      elif [[ $code -eq 0 && -s $dir/stdout.txt ]]; then
        painted_ok=1
      else
        painted_ok=0
      fi
      stop_groups "$window_pgid" "$grim_pgid"
      window_pid=
      window_pgid=
      grim_pgid=
    fi
  fi
  record window-read-today launch_bin today --date 2026-09-13 --json
  exit_is window-read-today 0
  stdout_has window-read-today '"v":1'
  stdout_has window-read-today "under way"
  record window-read-presets launch_bin presets --json
  exit_is window-read-presets 0
  stdout_has window-read-presets '"word":"depart"'
  runtime=$run/engine-probe-runtime
  mkdir -p "$runtime"
  chmod 700 "$runtime"
  inside_run "$runtime"
  record window-engine env \
    -u OMALOGBOOK_BIN \
    -u OMALOGBOOK_THEME_DIR \
    -u OMALOGBOOK_WIDTH \
    -u OMALOGBOOK_HEIGHT \
    HOME="$run/home" \
    XDG_CONFIG_HOME="$run/config" \
    XDG_RUNTIME_DIR="$runtime" \
    TZ=UTC \
    scripts/verify.sh engine
  dir=$run/artifacts/verify/window-engine
  if [[ -f $dir/stdout.txt ]] && grep -F -q 'Qt6 QML dev packages not installed.' "$dir/stdout.txt"; then
    printf '%s\n' 'Qt6 QML dev packages not installed.' >"$dir/miss.txt"
  fi
  if [[ -f $dir/stderr.txt ]] && grep -F -q 'Qt6 QML dev packages not installed.' "$dir/stderr.txt"; then
    printf '%s\n' 'Qt6 QML dev packages not installed.' >"$dir/miss.txt"
  fi
  if [[ -f $run/artifacts/verify/window-painted/miss.txt || -f $dir/miss.txt ]]; then
    return 1
  fi
  exit_is window-engine 0
  [[ $painted_ok -eq 1 ]]
  stdout_has window-engine "All engine-path checks passed."
}

probe_live_hub() {
  local art=$1 hub_runtime
  art_dir run >/dev/null
  hub_runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
  {
    command -v omakeel || echo "omakeel missing"
    if [[ -S $hub_runtime/omakeel/keel.sock ]]; then
      printf 'socket present %s\n' "$hub_runtime/omakeel/keel.sock"
    else
      echo "no keel socket"
    fi
  } | tee "$art/hub.txt"
  if [[ -S $hub_runtime/omakeel/keel.sock ]]; then
    SOCKET=$hub_runtime/omakeel/keel.sock
  else
    SOCKET=
  fi
}

drive_run() {
  local art dir ready=0
  art=$run/artifacts/verify/run
  mkdir -p "$art"
  probe_live_hub "$art"
  if [[ -z ${SOCKET:-} ]]; then
    printf '%s\n' 'omalogbook run --socket "$SOCKET" --no-git' >"$art/not-driven.txt"
    return 1
  fi
  reset_vault
  dir=$art
  mkdir -p "$dir"
  start_session "$dir/stdout.txt" "$dir/stderr.txt" \
    env \
    -u OMALOGBOOK_BIN \
    -u OMALOGBOOK_THEME_DIR \
    -u OMALOGBOOK_WIDTH \
    -u OMALOGBOOK_HEIGHT \
    HOME="$run/home" \
    XDG_CONFIG_HOME="$run/config" \
    XDG_RUNTIME_DIR="$run/runtime" \
    TZ=UTC \
    stdbuf -oL "$bin" run --socket "$SOCKET" --no-git
  follow_pid=$started_pid
  follow_pgid=$started_pid
  if ! run_ready "$dir"; then
    ready=1
  fi
  if [[ -n ${follow_pgid:-} ]]; then
    stop_groups "$follow_pgid"
  fi
  follow_pid=
  follow_pgid=
  return "$ready"
}

reject_exec() {
  local label=$1
  shift
  rm -f "$run/exec-sentinel"
  if "$@"; then
    printf 'accepted %s\n' "$label" >&2
    return 1
  fi
  if [[ -e $run/exec-sentinel ]]; then
    printf 'executed %s\n' "$label" >&2
    return 1
  fi
}

drive_confine() {
  local art saved real_bin outside canon marker esc1 esc2
  art=$(art_dir confine)
  saved=$(cat "$run/config/omalogbook/config.toml")
  real_bin=$bin
  cat >"$run/sentinel" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >> "$run/exec-sentinel"
EOF
  chmod +x "$run/sentinel"
  bin=$run/sentinel
  reset_vault
  rm -f "$run/exec-sentinel"
  launch_bin --version
  [[ -s $run/exec-sentinel ]]
  rm -f "$run/exec-sentinel"

  esc1=$run/../omalogbook-escape-a-$$
  esc2=$run/no-such/../../omalogbook-escape-b-$$
  write_config "$esc1"
  canon=$(canon_path "$esc1")
  [[ $canon != "$run"/* ]]
  reject_exec "doctor $esc1" require_vault
  reject_exec "drive $esc1" launch_bin vault
  [[ ! -e $canon ]]
  write_config "$esc2"
  canon=$(canon_path "$esc2")
  [[ $canon != "$run"/* ]]
  reject_exec "doctor $esc2" require_vault
  reject_exec "drive $esc2" launch_bin --version
  [[ ! -e $canon ]]

  outside=$(mktemp -d /tmp/omalogbook-outside.XXXXXX)
  rm -rf "$run/vault"
  ln -s "$outside" "$run/vault"
  write_config "$run/vault"
  canon=$(canon_path "$run/vault")
  [[ $canon == "$outside" ]]
  reject_exec "doctor symlink" require_vault
  reject_exec "drive symlink" launch_bin vault
  [[ -z $(find "$outside" -type f -print) ]]
  rm -rf "$run/vault" "$outside"
  reset_vault

  marker=$(mktemp /tmp/omalogbook-outside-write.XXXXXX)
  if confirm_drive; then
    rm -f "$marker"
    printf 'missed write %s\n' "$marker" >&2
    bin=$real_bin
    printf '%s\n' "$saved" >"$run/config/omalogbook/config.toml"
    return 1
  fi
  grep -F "$marker" "$run/outside-diff.txt" >/dev/null
  rm -f "$marker"
  confirm_drive

  bin=$real_bin
  printf '%s\n' "$saved" >"$run/config/omalogbook/config.toml"
  write_hub hub
  cat >"$run/inherit_fd.py" <<'PY'
import os
import socket
import subprocess
import sys
import time

art, run = sys.argv[1], sys.argv[2]
path = os.path.join(run, "inherit.sock")
try:
    os.remove(path)
except FileNotFoundError:
    pass
srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
srv.bind(path)
srv.listen(1)
cli = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
cli.connect(path)
conn, _ = srv.accept()
sleeper = subprocess.Popen(["sleep", "60"], pass_fds=(cli.fileno(),))
with open(os.path.join(art, "inherit.txt"), "w") as fh:
    fh.write(f"listener {os.getpid()}\n")
    fh.write(f"sleeper {sleeper.pid}\n")
    fh.write(f"socket {path}\n")
    fh.flush()
time.sleep(60)
PY
  start_session "$art/inherit.out" "$art/inherit.err" python3 "$run/inherit_fd.py" "$art" "$run"
  inherit_pgid=$started_pid
}

drive_pause() {
  local art
  art=$(art_dir pause)
  # The spawned pid ignores TERM. Only the cleanup KILL ends it.
  start_session "$art/stdout.txt" "$art/stderr.txt" \
    python3 -c 'import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(120)'
  pause_pid=$started_pid
  pause_pgid=$started_pid
  printf 'sleeper:%s\nrun:%s\n' "$pause_pid" "$run"
}

start_follow() {
  local art=$1 sock=$2
  SOCKET=$sock
  reset_vault
  mkdir -p "$art"
  start_session "$art/stdout.txt" "$art/stderr.txt" \
    env \
    -u OMALOGBOOK_BIN \
    -u OMALOGBOOK_THEME_DIR \
    -u OMALOGBOOK_WIDTH \
    -u OMALOGBOOK_HEIGHT \
    HOME="$run/home" \
    XDG_CONFIG_HOME="$run/config" \
    XDG_RUNTIME_DIR="$run/runtime" \
    TZ=UTC \
    stdbuf -oL "$bin" run --socket "$sock" --no-git
  follow_pid=$started_pid
  follow_pgid=$started_pid
}

finish_socket_cases() {
  local art sock listener sleeper note opened
  art=$run/artifacts/verify/confine
  sock=$run/inherit.sock
  for _ in $(seq 1 50); do
    [[ -f $art/inherit.txt ]] && break
    sleep 0.05
  done
  [[ -f $art/inherit.txt ]]
  listener=$(awk '/^listener / {print $2}' "$art/inherit.txt")
  sleeper=$(awk '/^sleeper / {print $2}' "$art/inherit.txt")
  [[ -n $listener && -n $sleeper ]]
  if pid_on_socket "$sleeper" "$sock"; then
    printf 'inherited client accepted\n' >&2
    return 1
  fi
  pid_on_socket "$listener" "$sock"
  stop_groups "$inherit_pgid"

  sock=$run/silent.sock
  rm -f "$sock"
  start_session "$art/silent-hub.out" "$art/silent-hub.err" python3 "$run/hub.py" silent "$sock"
  hub_pid=$started_pid
  hub_pgid=$started_pid
  for _ in $(seq 1 50); do
    [[ -S $sock ]] && break
    sleep 0.05
  done
  [[ -S $sock ]]
  start_follow "$art/silent-run" "$sock"
  if run_ready "$art/silent-run"; then
    printf 'silent accept counted as ready\n' >&2
    return 1
  fi
  grep -F 'ready:no' "$art/silent-run/ready.txt" >/dev/null
  if [[ -f $art/silent-run/stdout.txt ]] && grep -F -q 'log opened' "$art/silent-run/stdout.txt"; then
    printf 'silent hub wrote a log line\n' >&2
    return 1
  fi
  stop_groups "$follow_pgid" "$hub_pgid"
  follow_pid=
  hub_pid=

  sock=$run/protocol.sock
  rm -f "$sock"
  start_session "$art/protocol-hub.out" "$art/protocol-hub.err" python3 "$run/hub.py" protocol "$sock"
  hub_pid=$started_pid
  hub_pgid=$started_pid
  for _ in $(seq 1 50); do
    [[ -S $sock ]] && break
    sleep 0.05
  done
  [[ -S $sock ]]
  start_follow "$art/protocol-run" "$sock"
  run_ready "$art/protocol-run"
  note=$(note_path)
  opened=0
  for _ in $(seq 1 50); do
    if [[ -f $note && -f $art/protocol-run/stdout.txt ]] \
      && grep -F -q 'log opened' "$note" \
      && grep -F -q 'log opened' "$art/protocol-run/stdout.txt"; then
      opened=1
      break
    fi
    sleep 0.1
  done
  [[ $opened -eq 1 ]]
  stop_groups "$follow_pgid" "$hub_pgid"
  follow_pid=
  hub_pid=
}

if [[ $# -eq 0 ]]; then
  set -- doctor note today totals correct confine window run
fi

for feature in "$@"; do
  case $feature in
    doctor | note | today | totals | correct | window | run | confine | pause) ;;
    *)
      printf 'unknown feature: %s\n' "$feature" >&2
      exit 2
      ;;
  esac
done

if [[ $# -eq 1 && $1 == pause ]]; then
  prepare_bare
  drive_pause
  # Top level, not inside drive_pause. SIGINT in a function makes bash
  # drop that function context before the EXIT trap.
  sleep 120
  exit 0
fi

prepare
for feature in "$@"; do
  if [[ $feature == confine ]]; then
    drive_confine
    finish_socket_cases
  else
    "drive_$feature"
  fi
done
