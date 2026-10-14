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

- Run it with the drive in `../SKILL.md`. `vault` prints `$run/vault`.
- A live `omakeel`. This skill does not ship a replay, and it does not write a fake socket for this feature. `confine` is the stand-in that proves the predicate. It is not a watch.

`$run/runtime` has no hub. The drive probes the compositor's runtime directory. It does not point `XDG_RUNTIME_DIR` at that directory. `[ -S ]` only looks. It does not connect.

- **Do not start a blind follow.** When the socket is absent, the drive does not start `omalogbook run`. The command that was not run is `omalogbook run --socket "$SOCKET" --no-git`. `run` reconnects until it is killed. `run-follow` is not driven. The drive exits non-zero.
- **Follow, when the socket is there.** The drive empties `$run/vault` and starts `omalogbook run --socket "$SOCKET" --no-git` with `setsid`. It stops that follower before the next feature empties the vault.
- **Ready.** `run` prints `omalogbook:` and the note path, then calls `keel::follow`. That line is in stdout before the socket is read. On its own it is not a connection. Ready means all three of these.
  - `ss -lxp` shows the path in state `LISTEN`, and that pid holds the state `01` inode from `/proc/net/unix`.
  - A one-line read of the socket is JSON whose `type` is `hello`, `state`, or `targets`, and the object has `v`.
  - Stdout contains `omalogbook:` and the note path under `$run/vault`.
- A process that only inherited a connected client fd is not the listener. `pid_on_socket` returns non-zero for that pid. A hub that accepts and sends nothing has no protocol line, so the follow is not ready. `log opened` is retried for a few seconds, because the note line can land after ready returns.
- `Watch::no_fix` returns no lines when the watch has never had a fix. A hub that stays connected and never sends a position does not print `log opened`. A state line with a current fix prints `log opened`. `confine` uses that state line for the stand-in, and it requires `log opened` in stdout.
- **Proof.** Save `hub.txt`, `ready.txt` when the follow ran, and a copy of the note when one was written.

```sh
env -u BASH_ENV bash --noprofile --norc .cursor/skills/verify-omalogbook/drive.sh run
```

The stand-in is not this command. It runs inside `confine`.

```sh
env -u BASH_ENV bash --noprofile --norc .cursor/skills/verify-omalogbook/drive.sh confine
```

## Gotchas

- `--vault` on `run` does choose the folder. `--vault` on `note` does not. A `--vault` path that canonicalizes outside the run directory never starts the binary.
- `--no-git` is the flag for a drive. The config line `git = false` covers the same choice when the flag is absent.
- Do not point `--socket` at a file you created and call that a watch. `confine` does that only to prove the predicate, and the feature file for the watch stays `run`.
- `$run/runtime/omakeel/keel.sock` is the isolated runtime. A live hub is not there. `SOCKET` is the absolute path from the probe above.
- `mise replay` in this repo runs `cargo run -- run` against the default socket, not a temp one. A proof uses the binary and the run directory from `../SKILL.md`.
