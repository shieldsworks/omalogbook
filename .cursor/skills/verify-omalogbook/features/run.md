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

- Launch and Doctor from `../SKILL.md` are done. `omalogbook vault` prints `$run/vault`.
- `art="$run/artifacts/verify/run"`.
- A live `omakeel`. This skill does not ship a replay, and it does not write a fake socket.

`$run/env` points `XDG_RUNTIME_DIR` at `$run/runtime`, which has no hub. Probe the compositor's runtime directory from this shell. Do not apply `$run/env` to the shell first. `[ -S ]` only looks. It does not connect.

```sh
: "${run:?}" && mkdir -p "$art"
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
```

- **Do not start a blind follow.** When `SOCKET` is unset, stop. The command that was not run is `omalogbook run --socket "$SOCKET" --no-git`. `run` reconnects until it is killed, and a socket that never sends a fix produces no note. Report `run-follow` as not driven, with that command and the missing hub.
- **Follow, when the socket is there.** Empty `$run/vault` so this watch does not append to an earlier recipe. Guard in the foreground, then background the follow and write `$run/pid`.

```sh
: "${run:?}" && rm -rf "$run/vault" && mkdir -p "$run/vault"
: "${run:?}" && [ -r "$run/env" ] && [ -S "${SOCKET:?}" ] || exit 1
env $(cat "$run/env") ./target/debug/omalogbook run --socket "$SOCKET" --no-git >"$art/run.out" 2>"$art/run.err" &
printf '%s\n' "$!" > "$run/pid"
```

Ready when `$art/run.out` contains `log opened`, `under way`, `no fix:`, or `omakeel speaks protocol`. Those lines are printed from a hub message. The first line is `omalogbook:` plus the note path, and `run` prints it before it reads the socket, so that line is not readiness. Give the hub about 15 seconds. If the path line is still the only line, report `run-follow` as not driven. After a fix, the note on disk contains `log opened` or `under way`. Kill `$run/pid` in Cleanup either way.

```sh
: "${run:?}"
ready=
for _ in $(seq 1 15); do
  if grep -E 'log opened|under way|no fix:|omakeel speaks protocol' "$art/run.out" >/dev/null; then
    ready=yes
    break
  fi
  sleep 1
done
printf 'ready:%s\n' "${ready:-no}" | tee "$art/ready.txt"
```

- **Proof.** Save `hub.txt`, `ready.txt` when the follow ran, and a copy of the note when one was written. `: "${run:?}" && mkdir -p "${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/run" && cp -a "$art/." "${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/run/"`.

## Gotchas

- `--vault` on `run` does choose the folder. `--vault` on `note` does not. The two commands do not share that flag.
- `--no-git` is the flag for a drive. The config line `git = false` covers the same choice when the flag is absent.
- Do not point `--socket` at a file you created. A socket that is not omakeel is not a proof of the watch.
- `$run/runtime/omakeel/keel.sock` is the isolated runtime. A live hub is not there. `SOCKET` is the absolute path from the probe above.
- `mise replay` in this repo runs `cargo run -- run` against the default socket, not a temp one. A proof uses the binary and `$run/env` from `../SKILL.md`.
