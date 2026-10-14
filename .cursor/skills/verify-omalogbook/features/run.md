# Keep the watch

`omalogbook run` follows omakeel and writes the day's note, the day's run, and a GPX track for each passage. It writes nothing until the hub has a position.

## Sub-features

- `run-follow` tails the live `keel.sock`, passed with `--socket`.
- `run-absent` records that this machine has no socket, and does not start `run`.

## How to get to it (user POV)

- Run omakeel, then `omalogbook run` in a terminal. The log goes to the vault in `config.toml`.
- Run `omalogbook run --vault DIR --no-git` to write a throwaway vault without committing.
- Run `omalogbook run --socket PATH` when the hub is not on the default socket.
- Leave `run` going beside the window. The window does not keep the watch.

## Driving it with a live hub

Preconditions:

- Launch and Doctor from `../SKILL.md` are done. `drive vault` prints `$run/vault`.
- `art="$run/artifacts/verify/run"`.
- A live `omakeel`. This skill does not ship a replay, and it does not write a fake socket.

`$run/env` points `XDG_RUNTIME_DIR` at `$run/runtime`, which has no hub. Probe the compositor's runtime directory from this shell. Do not apply `$run/env` to the shell first. `[ -S ]` only looks. It does not connect.

```sh
probe_hub() {
  local saved=$- prev letters hub_runtime
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${art:-}" ] || return 1
  mkdir -p "$art" || return 1
  hub_runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
  {
    command -v omakeel || echo "omakeel missing"
    if [ -S "$hub_runtime/omakeel/keel.sock" ]; then
      printf 'socket present %s\n' "$hub_runtime/omakeel/keel.sock"
    else
      echo "no keel socket"
    fi
  } | tee "$art/hub.txt"
  if [ -S "$hub_runtime/omakeel/keel.sock" ]; then
    SOCKET=$hub_runtime/omakeel/keel.sock
  else
    unset SOCKET
  fi
}
probe_hub
```

- **Do not start a blind follow.** When `SOCKET` is unset, do not call `follow_hub`. The command that was not run is `omalogbook run --socket "$SOCKET" --no-git`. `run` reconnects until it is killed. Report `run-follow` as not driven, with that command and the missing hub.
- **Follow, when the socket is there.** Empty `$run/vault` so this watch does not append to an earlier recipe. `follow_hub` checks `run`, `env_pins_run`, and the socket, then backgrounds `omalogbook run` in the same `&&` list. A failed check returns 1 and does not append a pid. It does not start the binary, and it does not kill a pid already in `$run/pids`.

```sh
reset_run() {
  local saved=$- prev letters
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] || return 1
  rm -rf "$run/vault" || return 1
  mkdir -p "$run/vault" || return 1
}
```

```sh
follow_hub() {
  local saved=$- prev letters child
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] && [ -n "${art:-}" ] && env_pins_run && [ -S "${SOCKET:-}" ] && {
    env $(cat "$run/env") ./target/debug/omalogbook run --socket "$SOCKET" --no-git >"$art/run.out" 2>"$art/run.err" &
    child=$!
    printf '%s\n' "$child" >> "$run/pids" || { drop_pid "$child"; return 1; }
    hub_pid=$child
  }
}
```

When `SOCKET` is set, run `reset_run`, then `follow_hub`, then `run_ready`.

`run` prints `omalogbook:` and the note path, then calls `keel::follow`. That line is in stdout before the socket is read. On its own it is not a connection.

`Watch::no_fix` returns no lines when the watch has never had a fix. `outage` is still empty, and that branch writes nothing. A hub that stays connected and never sends a position does not print `log opened`, `under way`, `no fix:`, or `omakeel speaks protocol`. The log prints `log opened` from a fix, `no fix:` after a fix it already had goes away, and `omakeel speaks protocol` for a version it does not know.

`run_ready` is ready only when both of these are true.

- `$art/run.out` contains `omalogbook:` and the note path under `$run/vault`.
- `pid_on_socket` says the pid in `hub_pid` has a client fd for `$SOCKET`, and the process on the other end still holds a state `03` inode for that path.

`ss -xp` prints the peer inode as `0` on this kernel. A diag dump of the unix family returns `ENOENT`, so the peer column cannot pair the fds. `pidfd_getfd` duplicates this pid's socket fd. `getpeername` on that fd is `$SOCKET` only for the client of this path. `SO_PEERCRED` is the other process. That process has to hold the state `03` inode from `/proc/net/unix`. An inherited fd whose peer name is empty does not count. A state `03` row for the path from some other client does not count. After the listener and the accepted fd close, the inode is gone, and a pid that still has some other state `03` socket is not ready.

State `03` is an established unix connection. A live pid with only the path line is the reconnect loop. It is not a connection. Give the hub about 15 seconds. If `run_ready` returns 1, report `run-follow` as not driven. After a fix, the note on disk contains `log opened` or `under way`. `stop_drive` kills every pid in `$run/pids`.

`pid_on_socket` does not set a trap. `pidfd_open` is syscall 434 and `pidfd_getfd` is syscall 438 on x86_64 and aarch64.

```sh
pid_on_socket() {
  [ -n "${1:-}" ] || return 1
  [ -n "${SOCKET:-}" ] || return 1
  python3 - "$1" "$SOCKET" <<'PY'
import os
import socket
import struct
import sys
import ctypes

pid, path = sys.argv[1], sys.argv[2]
accepted = set()
with open("/proc/net/unix") as fh:
    next(fh)
    for line in fh:
        parts = line.split()
        if len(parts) >= 8 and parts[5] == "03" and parts[7] == path:
            accepted.add(parts[6])
if not accepted:
    raise SystemExit(1)
libc = ctypes.CDLL(None, use_errno=True)
try:
    names = os.listdir(f"/proc/{pid}/fd")
except OSError:
    raise SystemExit(1)
for name in names:
    try:
        target = os.readlink(f"/proc/{pid}/fd/{name}")
    except OSError:
        continue
    if not (target.startswith("socket:[") and target.endswith("]")):
        continue
    inode = target[len("socket:[") : -1]
    if inode in accepted:
        continue
    pidfd = libc.syscall(434, int(pid), 0)
    if pidfd < 0:
        continue
    got = libc.syscall(438, pidfd, int(name), 0)
    os.close(pidfd)
    if got < 0:
        continue
    try:
        held = socket.fromfd(got, socket.AF_UNIX, socket.SOCK_STREAM)
        peername = held.getpeername()
        cred = held.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
        peer = struct.unpack("III", cred)[0]
    except OSError:
        os.close(got)
        continue
    os.close(got)
    if peername != path:
        continue
    try:
        peer_names = os.listdir(f"/proc/{peer}/fd")
    except OSError:
        continue
    for pn in peer_names:
        try:
            pt = os.readlink(f"/proc/{peer}/fd/{pn}")
        except OSError:
            continue
        if pt.startswith("socket:[") and pt[len("socket:[") : -1] in accepted:
            raise SystemExit(0)
raise SystemExit(1)
PY
}
```

```sh
run_ready() {
  local saved=$- prev letters pid ready=no i
  prev=$(trap -p RETURN)
  trap 'if [ -n "$prev" ]; then eval "$prev"; else trap - RETURN; fi; letters=$(printf "%s" "$saved" | tr -d is); set +abefhkmnptuvxBCEHPT; if [ -n "$letters" ]; then set -"$letters"; fi' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${art:-}" ] || return 1
  [ -n "${SOCKET:-}" ] || return 1
  [ -n "${hub_pid:-}" ] || return 1
  pid=$hub_pid
  for i in $(seq 1 15); do
    if pid_on_socket "$pid" && grep -F "omalogbook: $run/vault/" "$art/run.out" >/dev/null; then
      ready=yes
      break
    fi
    sleep 1
  done
  printf 'ready:%s\n' "$ready" | tee "$art/ready.txt"
  [ "$ready" = yes ]
}
```

- **Proof.** Save `hub.txt`, `ready.txt` when the follow ran, and a copy of the note when one was written. `save_evidence run`.

## Gotchas

- `--vault` on `run` does choose the folder. `--vault` on `note` does not. The two commands do not share that flag.
- `--no-git` is the flag for a drive. The config line `git = false` covers the same choice when the flag is absent.
- Do not point `--socket` at a file you created. A socket that is not omakeel is not a proof of the watch.
- `$run/runtime/omakeel/keel.sock` is the isolated runtime. A live hub is not there. `SOCKET` is the absolute path from the probe above.
- `mise replay` in this repo runs `cargo run -- run` against the default socket, not a temp one. A proof uses the binary and `$run/env` from `../SKILL.md`.
