# Keep the watch

`omalogbook run` follows omakeel and writes the day's note, the day's run, and a GPX track for each passage. It writes nothing until the hub has a position.

## Sub-features

- `run-follow` tails `$XDG_RUNTIME_DIR/omakeel/keel.sock`, or the path given with `--socket`.
- `run-absent` records that this machine has no `omakeel` and no socket, and does not start `run`.

## How to get to it (user POV)

- Run omakeel, then `omalogbook run` in a terminal. The log goes to the vault in `config.toml`.
- Run `omalogbook run --vault DIR --no-git` to write a throwaway vault without committing.
- Run `omalogbook run --socket PATH` when the hub is not on the default socket.
- Leave `run` going beside the window. The window does not keep the watch.

## Driving it with a live hub

Preconditions:

- Launch and Doctor from `../SKILL.md` are done. `omalogbook vault` prints `$run/vault`.
- `art="$run/artifacts/verify/run"; mkdir -p "$art"`.
- A live `omakeel` whose socket you can point at with `--socket`. This skill does not ship a replay, and it does not write a fake socket.

- **Look for the hub.** Record the precondition.

```sh
{
  command -v omakeel || echo "omakeel missing"
  if [ -S "$run/runtime/omakeel/keel.sock" ]; then
    echo "socket present"
  else
    echo "no keel socket"
  fi
} | tee "$art/hub.txt"
```

- **Do not start a blind follow.** When `hub.txt` says `omakeel missing` or `no keel socket`, stop. The command that was not run is `omalogbook run --no-git`. `run` reconnects until it is killed, and a socket that never sends a fix produces no note. Report `run-follow` as not driven, with that command and the missing hub.
- **Follow, when a hub is actually up.** Start `omalogbook run --socket "$SOCKET" --no-git >"$art/run.out" 2>"$art/run.err" &` and write the pid to `$run/pid`. Ready when stdout contains the note path under `$run/vault`. After a fix, the note on disk contains `log opened` or `under way`. Kill only `$run/pid` in Cleanup.
- **Proof.** Save `hub.txt` and, when the follow ran, the log and a copy of the note. Copy `$art` to `${EVIDENCE_DIR:-/tmp/omalogbook-evidence}/run/` before cleanup. Observed on 2026-10-14. `hub.txt` was `omakeel missing` and `no keel socket`. `omalogbook run` was not started.

## Gotchas

- `--vault` on `run` does choose the folder. `--vault` on `note` does not. The two commands do not share that flag.
- `--no-git` is the flag for a drive. The config line `git = false` covers the same choice when the flag is absent.
- Do not point `--socket` at a file you created. A socket that is not omakeel is not a proof of the watch.
- `mise replay` in this repo runs `cargo run -- run` against the default socket, not a temp one. A proof uses the binary and `$run/env` from `../SKILL.md`.
