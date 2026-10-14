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
  local opts hub_runtime
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
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
- **Follow, when the socket is there.** Empty `$run/vault` so this watch does not append to an earlier recipe. `follow_hub` checks `run`, `$run/env`, and the socket, then backgrounds `omalogbook run` in the same function. A failed check calls `stop_pid` and returns 1. It does not start the binary.

```sh
reset_run() {
  local opts
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  [ -n "${run:-}" ] || return 1
  rm -rf "$run/vault" || return 1
  mkdir -p "$run/vault" || return 1
}
```

```sh
follow_hub() {
  local opts
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  if [ -z "${run:-}" ] || [ -z "${art:-}" ] || [ ! -r "$run/env" ] || [ ! -S "${SOCKET:-}" ]; then
    stop_pid
    return 1
  fi
  env $(cat "$run/env") ./target/debug/omalogbook run --socket "$SOCKET" --no-git >"$art/run.out" 2>"$art/run.err" &
  printf '%s\n' "$!" > "$run/pid" || {
    kill "$!" 2>/dev/null || true
    wait "$!" 2>/dev/null || true
    return 1
  }
}
```

When `SOCKET` is set, run `reset_run`, then `follow_hub`, then `run_ready`.

`run` prints `omalogbook:` and the note path, then calls `keel::follow`. That line is in stdout before the socket is read. On its own it is not a connection.

`Watch::no_fix` returns no lines when the watch has never had a fix. `outage` is still empty, and that branch writes nothing. A hub that stays connected and never sends a position does not print `log opened`, `under way`, `no fix:`, or `omakeel speaks protocol`. The log prints `log opened` from a fix, `no fix:` after a fix it already had goes away, and `omakeel speaks protocol` for a version it does not know.

`run_ready` is ready only when both of these are true.

- `$art/run.out` contains `omalogbook:` and the note path under `$run/vault`.
- `/proc/net/unix` lists `$SOCKET` in state `03`, and the pid in `$run/pid` has a socket fd in that same state.

State `03` is an established unix connection. A live pid with only the path line, and no state `03` for `$SOCKET`, is the reconnect loop. It is not a connection. Give the hub about 15 seconds. If `run_ready` returns 1, report `run-follow` as not driven. After a fix, the note on disk contains `log opened` or `under way`. `stop_drive` stops `$run/pid` either way.

```sh
run_ready() {
  local opts pid ready=no i fd target inode state connected
  opts=$(set +o)
  trap 'eval "$opts"; trap - RETURN' RETURN
  [ -n "${run:-}" ] || return 1
  [ -n "${art:-}" ] || return 1
  [ -n "${SOCKET:-}" ] || return 1
  [ -f "$run/pid" ] || return 1
  pid=$(cat "$run/pid") || return 1
  for i in $(seq 1 15); do
    connected=no
    if awk -v sock="$SOCKET" '$6 == "03" && $8 == sock { found = 1 } END { exit found ? 0 : 1 }' /proc/net/unix; then
      for fd in /proc/"$pid"/fd/*; do
        target=$(readlink "$fd" 2>/dev/null || true)
        case $target in
          socket:\[*)
            inode=${target#socket:[}
            inode=${inode%]}
            state=$(awk -v ino="$inode" '$7 == ino { print $6; exit }' /proc/net/unix)
            if [ "$state" = "03" ]; then
              connected=yes
              break
            fi
            ;;
        esac
      done
    fi
    if [ "$connected" = yes ] && grep -F "omalogbook: $run/vault/" "$art/run.out" >/dev/null; then
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
